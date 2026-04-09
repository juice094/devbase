use chrono::{Utc, Duration};

pub fn generate_daily_digest(conn: &rusqlite::Connection, config: &crate::config::Config) -> anyhow::Result<String> {
    let since = (Utc::now() - Duration::hours(config.digest.window_hours)).to_rfc3339();
    let mut lines = Vec::new();
    lines.push(crate::i18n::current().log.digest_title.to_string());
    lines.push(format!("{}: {}", crate::i18n::current().log.digest_generated_at, Utc::now().format("%Y-%m-%d %H:%M UTC")));
    lines.push("".to_string());

    // 1. 新入库仓库统计
    let new_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM repos WHERE discovered_at > ?1",
        [&since],
        |row| row.get(0),
    )?;
    if new_count > 0 {
        lines.push(format!("{}: {} repos", crate::i18n::current().log.digest_new_repos, new_count));
        let mut stmt = conn.prepare("SELECT id FROM repos WHERE discovered_at > ?1 ORDER BY discovered_at DESC")?;
        let ids = stmt.query_map([&since], |row| row.get::<_, String>(0))?;
        for id in ids.take(5) {
            lines.push(format!("  - {}", id?));
        }
        lines.push("".to_string());
    }

    // 2. 健康状态异常的仓库（behind > 0 或 dirty）
    // 从 repo_health 表读取，并 JOIN repo_summaries 获取摘要
    let mut stmt = conn.prepare(
        "SELECT h.repo_id, h.status, h.ahead, h.behind, s.summary
         FROM repo_health h
         LEFT JOIN repo_summaries s ON h.repo_id = s.repo_id
         WHERE h.status IN ('dirty', 'behind', 'diverged', 'ahead')
         ORDER BY h.behind DESC, h.repo_id"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut unhealthy = Vec::new();
    for row in rows {
        unhealthy.push(row?);
    }
    if !unhealthy.is_empty() {
        lines.push(format!("{}: ({})", crate::i18n::current().log.digest_unhealthy_repos, unhealthy.len()));
        for (id, status, ahead, behind, summary_opt) in unhealthy.iter().take(10) {
            let summary = summary_opt.as_deref().unwrap_or(crate::i18n::current().log.digest_no_summary);
            lines.push(format!("  [{}] status={} ahead={} behind={} | {}", id, status, ahead, behind, summary));
        }
        lines.push("".to_string());
    }

    // 3. 新发现（ai_discoveries）
    let disc_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ai_discoveries WHERE timestamp > ?1",
        [&since],
        |row| row.get(0),
    )?;
    if disc_count > 0 {
        lines.push(format!("{}: ({})", crate::i18n::current().log.digest_new_discoveries, disc_count));
        let mut stmt = conn.prepare("SELECT repo_id, discovery_type, description FROM ai_discoveries WHERE timestamp > ?1 ORDER BY timestamp DESC LIMIT 5")?;
        let rows = stmt.query_map([&since], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (repo_id_opt, dtype, desc) = row?;
            let repo = repo_id_opt.unwrap_or_else(|| crate::i18n::current().log.digest_global.to_string());
            lines.push(format!("  [{}] {}: {}", repo, dtype, desc));
        }
        lines.push("".to_string());
    }

    // 4. 总体统计
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM repos", [], |row| row.get(0))?;
    let synced: i64 = conn.query_row("SELECT COUNT(*) FROM repo_health WHERE checked_at > ?1", [&since], |row| row.get(0))?;
    lines.push(format!("{}: {} repos in db, {} checked in past 24h", crate::i18n::current().log.digest_overall, total, synced));

    Ok(lines.join("\n"))
}
