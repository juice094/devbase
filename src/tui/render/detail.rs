use crate::tui::theme::Styles;
use crate::tui::{App, DetailTab, MainView};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Sparkline, Tabs, Wrap},
};

pub(crate) fn render_detail(frame: &mut Frame, app: &mut App, area: Rect, styles: &Styles) {
    match app.main_view {
        MainView::RepoList => render_repo_detail(frame, app, area, styles),
        MainView::VaultList => render_vault_detail(frame, app, area, styles),
    }
}

fn render_repo_detail(frame: &mut Frame, app: &mut App, area: Rect, styles: &Styles) {
    let repo = match app.current_repo() {
        Some(r) => r.clone(),
        None => {
            let msg = Paragraph::new(crate::i18n::current().log.no_repos_registered).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(crate::i18n::current().tui.title_details)
                    .border_style(styles.border),
            );
            frame.render_widget(msg, area);
            return;
        }
    };

    let i18n = crate::i18n::current();

    // Split area into tabs header + content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Tab bar
    let titles = vec![
        Line::from(DetailTab::Overview.label()),
        Line::from(DetailTab::Health.label()),
        Line::from(DetailTab::Insights.label()),
    ];
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(i18n.tui.title_details)
                .border_style(styles.border),
        )
        .select(app.detail_tab as usize)
        .highlight_style(Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD))
        .divider(symbols::line::VERTICAL);
    frame.render_widget(tabs, chunks[0]);

    let content_area = chunks[1];

    match app.detail_tab {
        DetailTab::Overview => render_overview(frame, &repo, content_area, styles),
        DetailTab::Health => render_health(frame, &repo, content_area, styles),
        DetailTab::Insights => render_insights(frame, app, &repo, content_area, styles),
    }
}

// ---------------------------------------------------------------------------
// Overview tab
// ---------------------------------------------------------------------------

fn render_overview(frame: &mut Frame, repo: &crate::tui::RepoItem, area: Rect, styles: &Styles) {
    let (dirty, ahead, behind) = match (repo.status_dirty, repo.status_ahead, repo.status_behind) {
        (Some(d), Some(a), Some(b)) => (d, a, b),
        _ => (false, 0, 0),
    };

    let status_color = if dirty {
        styles.theme.danger
    } else if behind > 0 || ahead > 0 {
        styles.theme.warning
    } else {
        styles.theme.success
    };
    let status_icon = if dirty {
        "⚠"
    } else if behind > 0 || ahead > 0 {
        "●"
    } else {
        "✓"
    };
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

    let head_short = super::read_head_commit(&repo.local_path).unwrap_or_else(|| "—".to_string());
    let (last_sync_human, last_sync_action, last_sync_commit) =
        super::read_syncdone_info(&repo.local_path);
    let summary_text = super::read_repo_summary(&repo.id).unwrap_or_else(|| "暂无描述".to_string());

    let policy = crate::sync::SyncPolicy::from_tags(&repo.tags.join(","));
    let policy_text = format!("{:?}", policy);
    let policy_color = match policy {
        crate::sync::SyncPolicy::Mirror => styles.theme.danger,
        crate::sync::SyncPolicy::Conservative => styles.theme.warning,
        crate::sync::SyncPolicy::Rebase => styles.theme.success,
        crate::sync::SyncPolicy::Merge => Color::Magenta,
    };

    let mut tag_line = vec![Span::styled("标签: ", styles.label)];
    tag_line.extend(crate::tui::tag_spans(&repo.tags));

    // Horizontal rule for visual separation between layers.
    let hr = Span::styled(
        "─".repeat(area.width.saturating_sub(2) as usize),
        Style::default().fg(styles.theme.border),
    );

    let mut lines: Vec<Line> = vec![
        // === Layer 1: Identity + core status ===
        Line::from(vec![
            Span::styled(
                format!("{} ", status_icon),
                Style::default().fg(status_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &repo.id,
                Style::default().fg(styles.theme.text).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("    "),
            Span::styled(status_desc, Style::default().fg(status_color)),
        ]),
        Line::from(vec![hr.clone()]),
        // === Layer 2: Commit + policy ===
        Line::from(vec![
            Span::styled("HEAD: ", styles.dim),
            Span::styled(head_short, Style::default().fg(styles.theme.text)),
            Span::styled("  策略: ", styles.dim),
            Span::styled(policy_text, Style::default().fg(policy_color)),
        ]),
    ];

    // Mirror policy callout — separated from the policy line so it doesn't
    // feel visually cramped.
    if policy == crate::sync::SyncPolicy::Mirror {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  ▐ ", Style::default().fg(styles.theme.danger)),
            Span::styled(
                crate::i18n::current().sync.mirror_policy_warning,
                Style::default().fg(styles.theme.danger),
            ),
        ]));
        lines.push(Line::from(""));
    } else {
        lines.push(Line::from(""));
    }

    lines.extend(vec![
        // === Layer 3: Description ===
        Line::from(vec![
            Span::styled("描述: ", styles.label),
            Span::styled(summary_text, Style::default().fg(styles.theme.text)),
        ]),
        Line::from(vec![hr.clone()]),
        // === Layer 4: Connection metadata ===
        Line::from(vec![
            Span::styled("分支: ", styles.label),
            Span::raw(repo.default_branch.as_deref().unwrap_or("—")),
            Span::styled("  语言: ", styles.label),
            Span::raw(repo.language.as_deref().unwrap_or("—")),
        ]),
        Line::from(vec![
            Span::styled("远程: ", styles.label),
            Span::styled(
                repo.upstream_url.as_deref().unwrap_or("(无)"),
                if repo.upstream_url.is_some() {
                    styles.link
                } else {
                    Style::default().fg(styles.theme.warning)
                },
            ),
        ]),
        Line::from(tag_line),
        Line::from(vec![hr.clone()]),
        // === Layer 5: Sync history ===
        Line::from(vec![
            Span::styled("上次同步: ", styles.dim),
            Span::styled(last_sync_human, Style::default().fg(styles.theme.text)),
            Span::styled(format!(" ({}) ", last_sync_action), styles.dim),
            Span::styled(last_sync_commit, styles.dim),
        ]),
        Line::from(""),
        // === Layer 6: Action hint ===
        Line::from(vec![
            Span::styled("操作: ", styles.dim),
            Span::styled("s 预览  S 执行  r 刷新", styles.dim),
        ]),
    ]);

    let para = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).border_style(styles.border))
        .wrap(Wrap { trim: true });
    frame.render_widget(para, area);
}

// ---------------------------------------------------------------------------
// Health tab
// ---------------------------------------------------------------------------

fn render_health(frame: &mut Frame, repo: &crate::tui::RepoItem, area: Rect, styles: &Styles) {
    let (status, ahead, behind) = crate::health::analyze_repo(
        &repo.local_path,
        repo.upstream_url.as_deref(),
        repo.default_branch.as_deref(),
    );

    let status_lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("仓库: ", styles.label),
            Span::styled(
                &repo.id,
                Style::default().fg(styles.theme.text).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("路径: ", styles.label),
            Span::styled(&repo.local_path, styles.dim),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("健康状态: ", styles.label),
            health_status_span(&status, styles),
        ]),
        Line::from(vec![
            Span::styled("Ahead: ", styles.label),
            Span::styled(format!("{}", ahead), Style::default().fg(styles.theme.text)),
        ]),
        Line::from(vec![
            Span::styled("Behind: ", styles.label),
            Span::styled(format!("{}", behind), Style::default().fg(styles.theme.text)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(health_explanation(&status), styles.dim)]),
    ];

    let para = Paragraph::new(Text::from(status_lines))
        .block(Block::default().borders(Borders::ALL).border_style(styles.border))
        .wrap(Wrap { trim: true });
    frame.render_widget(para, area);
}

fn health_status_span<'a>(status: &'a str, styles: &'a Styles) -> Span<'a> {
    let (text, color) = match status {
        "ok" | "clean" => ("✓ 正常", styles.theme.success),
        "dirty" => ("● 工作目录不干净", styles.theme.warning),
        "diverged" => ("◆ 本地与远程分叉", styles.theme.danger),
        "detached" => ("⚠ HEAD 分离", styles.theme.warning),
        "no_upstream" => ("○ 无上游仓库", styles.theme.muted),
        "error" => ("✗ 错误", styles.theme.danger),
        _ => ("? 未知", styles.theme.muted),
    };
    Span::styled(text, Style::default().fg(color).add_modifier(Modifier::BOLD))
}

fn health_explanation(status: &str) -> String {
    match status {
        "ok" => "该仓库工作目录干净，且与远程保持同步。".to_string(),
        "clean" => "该仓库工作目录干净。".to_string(),
        "dirty" => "工作目录存在未提交的修改。建议先提交或暂存更改后再进行同步操作。".to_string(),
        "diverged" => "本地分支与远程分支已经分叉，存在冲突风险。需要手动合并或变基。".to_string(),
        "detached" => "当前处于分离 HEAD 状态。建议检出到一个分支上再进行操作。".to_string(),
        "no_upstream" => "该仓库没有配置上游远程地址，属于本地-only 项目。".to_string(),
        "error" => "打开仓库时发生错误，请检查路径是否有效。".to_string(),
        _ => "无法判断该仓库的健康状态。".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Insights tab
// ---------------------------------------------------------------------------

fn render_insights(
    frame: &mut Frame,
    app: &App,
    repo: &crate::tui::RepoItem,
    area: Rect,
    styles: &Styles,
) {
    let insights = app.generate_insights(repo);
    let history = if let Ok(conn) = crate::registry::WorkspaceRegistry::init_db() {
        crate::registry::WorkspaceRegistry::get_stars_history(&conn, &repo.id, 30)
            .unwrap_or_default()
    } else {
        vec![]
    };
    let has_enough = history.len() >= 2;

    let mut all_lines: Vec<Line> = Vec::new();

    if !insights.is_empty() {
        all_lines.push(Line::from(vec![Span::styled("洞察", styles.label)]));
        all_lines.push(Line::from(""));
        for text in insights.iter().take(5) {
            all_lines.push(Line::from(Span::styled(
                text.clone(),
                Style::default().fg(styles.theme.primary),
            )));
        }
        all_lines.push(Line::from(""));
    }

    if has_enough {
        all_lines.push(Line::from(vec![Span::styled("Stars Trend (30天)", styles.label)]));
        all_lines.push(Line::from(""));
    }

    let spark_height = if has_enough { 4 } else { 0 };
    let text_height = all_lines.len() as u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(text_height.min(area.height.saturating_sub(spark_height))),
            Constraint::Length(spark_height),
        ])
        .split(area);

    let para = Paragraph::new(Text::from(all_lines))
        .block(Block::default().borders(Borders::ALL).border_style(styles.border))
        .wrap(Wrap { trim: true });
    frame.render_widget(para, chunks[0]);

    if has_enough && chunks[1].height > 0 {
        let spark_data: Vec<u64> = history.iter().map(|(s, _)| *s).collect();
        let max_stars = spark_data.iter().max().copied().unwrap_or(1).max(1);
        let current = spark_data.last().copied().unwrap_or(0);
        let first = spark_data.first().copied().unwrap_or(0);
        let delta = current as i64 - first as i64;
        let delta_text = if delta >= 0 {
            format!("(+{})", delta)
        } else {
            format!("({})", delta)
        };
        let delta_color = if delta >= 0 {
            styles.theme.success
        } else {
            styles.theme.danger
        };

        let spark_text_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(chunks[1]);

        let sparkline = Sparkline::default()
            .data(&spark_data)
            .max(max_stars)
            .style(Style::default().fg(styles.theme.info));
        frame.render_widget(sparkline, spark_text_chunks[0]);

        let label = Paragraph::new(Line::from(vec![
            Span::styled(format!("★{} ", current), styles.star),
            Span::styled(delta_text, Style::default().fg(delta_color)),
        ]));
        frame.render_widget(label, spark_text_chunks[1]);
    }
}

// ---------------------------------------------------------------------------
// Vault detail
// ---------------------------------------------------------------------------

fn render_vault_detail(frame: &mut Frame, app: &mut App, area: Rect, styles: &Styles) {
    let vault = match app.current_vault() {
        Some(v) => v.clone(),
        None => {
            let msg = Paragraph::new("没有选中的笔记").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("笔记详情")
                    .border_style(styles.border),
            );
            frame.render_widget(msg, area);
            return;
        }
    };

    let title = vault.title.as_deref().unwrap_or(&vault.id);

    // Read content from filesystem
    let content_preview = crate::vault::fs_io::read_note_body(&vault.path)
        .map(|(body, _fm)| {
            let preview: String = body.lines().take(20).collect::<Vec<_>>().join("\n");
            preview
        })
        .unwrap_or_else(|| "无法读取笔记内容".to_string());

    let tags_text = if vault.tags.is_empty() {
        "(无标签)".to_string()
    } else {
        vault.tags.join(", ")
    };

    let links_text = if vault.outgoing_links.is_empty() {
        "(无出站链接)".to_string()
    } else {
        vault.outgoing_links.join(", ")
    };

    let text = Text::from(vec![
        Line::from(vec![
            Span::styled("标题: ", Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD)),
            Span::raw(title),
        ]),
        Line::from(vec![
            Span::styled("路径: ", Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD)),
            Span::styled(&vault.path, styles.dim),
        ]),
        Line::from(vec![
            Span::styled("标签: ", Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD)),
            Span::raw(&tags_text),
        ]),
        Line::from(vec![
            Span::styled("链接: ", Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD)),
            Span::raw(&links_text),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("预览:", Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(&content_preview, styles.dim)),
    ]);

    // Backlinks section
    let vault_dir = crate::registry::WorkspaceRegistry::workspace_dir()
        .ok()
        .map(|ws| ws.join("vault"));
    let backlinks = if let Some(vd) = vault_dir {
        match crate::vault::backlinks::build_backlink_index(&vd) {
            Ok(index) => crate::vault::backlinks::get_backlinks(&index, &vault.id),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };

    let mut all_lines = text.lines.to_vec();
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(vec![
        Span::styled("被引用: ", Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD)),
        Span::styled(
            if backlinks.is_empty() {
                "(无)".to_string()
            } else {
                format!("{} 篇笔记", backlinks.len())
            },
            styles.dim,
        ),
    ]));
    for src in backlinks.iter().take(10) {
        all_lines.push(Line::from(vec![
            Span::raw("  ← "),
            Span::styled(src, Style::default().fg(styles.theme.info)),
        ]));
    }
    if backlinks.len() > 10 {
        all_lines.push(Line::from(vec![
            Span::styled(format!("  ... 还有 {} 篇", backlinks.len() - 10), styles.dim),
        ]));
    }

    let text_with_backlinks = Text::from(all_lines);

    let paragraph = Paragraph::new(text_with_backlinks)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("笔记: {}", title))
                .border_style(styles.border)
                .padding(ratatui::widgets::Padding::horizontal(1)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}
