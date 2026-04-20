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
