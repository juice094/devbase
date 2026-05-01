use chrono::{Duration, Utc};

pub fn generate_daily_digest(
    conn: &rusqlite::Connection,
    config: &crate::config::Config,
    i18n: &crate::i18n::I18n,
) -> anyhow::Result<String> {
    let since = (Utc::now() - Duration::hours(config.digest.window_hours)).to_rfc3339();
    let mut lines = Vec::new();
    lines.push(i18n.log.digest_title.to_string());
    lines.push(format!(
        "{}: {}",
        i18n.log.digest_generated_at,
        Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));
    lines.push("".to_string());

    // 1. 新入库仓库统计
    let new_count: i64 =
        conn.query_row(&format!("SELECT COUNT(*) FROM entities WHERE entity_type = '{}' AND json_extract(metadata, '$.discovered_at') > ?1", crate::registry::ENTITY_TYPE_REPO), [&since], |row| {
            row.get(0)
        })?;
    if new_count > 0 {
        lines.push(format!("{}: {} repos", i18n.log.digest_new_repos, new_count));
        let mut stmt = conn
            .prepare(&format!("SELECT id FROM entities WHERE entity_type = '{}' AND json_extract(metadata, '$.discovered_at') > ?1 ORDER BY json_extract(metadata, '$.discovered_at') DESC", crate::registry::ENTITY_TYPE_REPO))?;
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
         ORDER BY h.behind DESC, h.repo_id",
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
        lines.push(format!("{}: ({})", i18n.log.digest_unhealthy_repos, unhealthy.len()));
        for (id, status, ahead, behind, summary_opt) in unhealthy.iter().take(10) {
            let summary = summary_opt.as_deref().unwrap_or(i18n.log.digest_no_summary);
            lines.push(format!(
                "  [{}] status={} ahead={} behind={} | {}",
                id, status, ahead, behind, summary
            ));
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
        lines.push(format!("{}: ({})", i18n.log.digest_new_discoveries, disc_count));
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
            let repo = repo_id_opt.unwrap_or_else(|| i18n.log.digest_global.to_string());
            lines.push(format!("  [{}] {}: {}", repo, dtype, desc));
        }
        lines.push("".to_string());
    }

    // 4. 总体统计
    let total: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM entities WHERE entity_type = '{}'",
            crate::registry::ENTITY_TYPE_REPO
        ),
        [],
        |row| row.get(0),
    )?;
    let synced: i64 = conn.query_row(
        "SELECT COUNT(*) FROM repo_health WHERE checked_at > ?1",
        [&since],
        |row| row.get(0),
    )?;
    lines.push(format!(
        "{}: {} repos in db, {} checked in past 24h",
        i18n.log.digest_overall, total, synced
    ));

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WorkspaceRegistry;

    fn default_config() -> crate::config::Config {
        crate::config::Config::default()
    }

    #[test]
    fn test_generate_daily_digest_empty() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let config = default_config();
        let i18n = crate::i18n::from_language("en");
        let digest = generate_daily_digest(&conn, &config, &i18n).unwrap();
        assert!(digest.contains("Daily Digest"));
        assert!(digest.contains("0 repos in db"));
    }

    #[test]
    fn test_generate_daily_digest_with_repos() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let repo = crate::registry::RepoEntry {
            id: "repo1".to_string(),
            local_path: std::path::PathBuf::from("/tmp/repo1"),
            tags: vec![],
            discovered_at: Utc::now(),
            language: Some("rust".to_string()),
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };
        crate::registry::repo::save_repo(&mut conn, &repo).unwrap();

        let config = default_config();
        let i18n = crate::i18n::from_language("en");
        let digest = generate_daily_digest(&conn, &config, &i18n).unwrap();
        assert!(digest.contains("1 repos in db"));
    }

    #[test]
    fn test_generate_daily_digest_with_unhealthy_repo() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let repo = crate::registry::RepoEntry {
            id: "sick_repo".to_string(),
            local_path: std::path::PathBuf::from("/tmp/sick"),
            tags: vec![],
            discovered_at: Utc::now(),
            language: None,
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };
        crate::registry::repo::save_repo(&mut conn, &repo).unwrap();
        conn.execute(
            "INSERT INTO repo_health (repo_id, status, ahead, behind, checked_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["sick_repo", "dirty", 0, 2, Utc::now().to_rfc3339()],
        ).unwrap();

        let config = default_config();
        let i18n = crate::i18n::from_language("en");
        let digest = generate_daily_digest(&conn, &config, &i18n).unwrap();
        assert!(digest.contains("Repositories needing attention"));
        assert!(digest.contains("sick_repo"));
        assert!(digest.contains("status=dirty"));
    }
}
