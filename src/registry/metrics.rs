use super::*;
use chrono::{DateTime, Utc};

impl WorkspaceRegistry {
    pub fn save_code_metrics(
        conn: &rusqlite::Connection,
        repo_id: &str,
        metrics: &CodeMetrics,
    ) -> anyhow::Result<()> {
        let language_breakdown = serde_json::to_string(&metrics.language_breakdown)?;
        conn.execute(
            "INSERT OR REPLACE INTO repo_code_metrics
             (repo_id, total_lines, source_lines, test_lines, comment_lines, file_count, language_breakdown, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![repo_id, metrics.total_lines as i64, metrics.source_lines as i64, metrics.test_lines as i64,
                              metrics.comment_lines as i64, metrics.file_count as i64, language_breakdown,
                              chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn get_code_metrics(
        conn: &rusqlite::Connection,
        repo_id: &str,
    ) -> anyhow::Result<Option<CodeMetrics>> {
        let mut stmt =
            conn.prepare("SELECT total_lines, source_lines, test_lines, comment_lines, file_count, language_breakdown, updated_at FROM repo_code_metrics WHERE repo_id = ?1")?;
        let mut rows = stmt.query([repo_id])?;
        if let Some(row) = rows.next()? {
            let total_lines: i64 = row.get(0)?;
            let source_lines: i64 = row.get(1)?;
            let test_lines: i64 = row.get(2)?;
            let comment_lines: i64 = row.get(3)?;
            let file_count: i64 = row.get(4)?;
            let language_breakdown: String = row.get(5)?;
            let updated_at: String = row.get(6)?;
            let language_breakdown =
                serde_json::from_str(&language_breakdown).unwrap_or(serde_json::Value::Null);
            let updated_at = DateTime::parse_from_rfc3339(&updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok(Some(CodeMetrics {
                total_lines: total_lines as usize,
                source_lines: source_lines as usize,
                test_lines: test_lines as usize,
                comment_lines: comment_lines as usize,
                file_count: file_count as usize,
                language_breakdown,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_code_metrics(
        conn: &rusqlite::Connection,
    ) -> anyhow::Result<Vec<(String, CodeMetrics)>> {
        let mut stmt = conn.prepare(
            "SELECT repo_id, total_lines, source_lines, test_lines, comment_lines, file_count, language_breakdown, updated_at FROM repo_code_metrics"
        )?;
        let rows = stmt.query_map([], |row| {
            let repo_id: String = row.get(0)?;
            let total_lines: i64 = row.get(1)?;
            let source_lines: i64 = row.get(2)?;
            let test_lines: i64 = row.get(3)?;
            let comment_lines: i64 = row.get(4)?;
            let file_count: i64 = row.get(5)?;
            let language_breakdown: String = row.get(6)?;
            let updated_at: String = row.get(7)?;
            let language_breakdown =
                serde_json::from_str(&language_breakdown).unwrap_or(serde_json::Value::Null);
            let updated_at = DateTime::parse_from_rfc3339(&updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok((
                repo_id,
                CodeMetrics {
                    total_lines: total_lines as usize,
                    source_lines: source_lines as usize,
                    test_lines: test_lines as usize,
                    comment_lines: comment_lines as usize,
                    file_count: file_count as usize,
                    language_breakdown,
                    updated_at,
                },
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_metrics() -> CodeMetrics {
        CodeMetrics {
            total_lines: 1000,
            source_lines: 700,
            test_lines: 200,
            comment_lines: 100,
            file_count: 10,
            language_breakdown: serde_json::json!({"rust": 500, "python": 300}),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_save_and_get_code_metrics() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        let metrics = sample_metrics();
        WorkspaceRegistry::save_code_metrics(&conn, "repo-a", &metrics).unwrap();

        let fetched = WorkspaceRegistry::get_code_metrics(&conn, "repo-a").unwrap().unwrap();
        assert_eq!(fetched.total_lines, 1000);
        assert_eq!(fetched.source_lines, 700);
        assert_eq!(fetched.test_lines, 200);
        assert_eq!(fetched.comment_lines, 100);
        assert_eq!(fetched.file_count, 10);
        assert_eq!(fetched.language_breakdown, serde_json::json!({"rust": 500, "python": 300}));
    }

    #[test]
    fn test_get_code_metrics_missing() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let result = WorkspaceRegistry::get_code_metrics(&conn, "missing").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_code_metrics() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-b").unwrap();
        let m1 = sample_metrics();
        let mut m2 = sample_metrics();
        m2.total_lines = 500;
        WorkspaceRegistry::save_code_metrics(&conn, "repo-a", &m1).unwrap();
        WorkspaceRegistry::save_code_metrics(&conn, "repo-b", &m2).unwrap();

        let all = WorkspaceRegistry::list_code_metrics(&conn).unwrap();
        assert_eq!(all.len(), 2);
        let total: usize = all.iter().map(|(_, m)| m.total_lines).sum();
        assert_eq!(total, 1500);
    }

    #[test]
    fn test_list_code_metrics_empty() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let all = WorkspaceRegistry::list_code_metrics(&conn).unwrap();
        assert!(all.is_empty());
    }
}
