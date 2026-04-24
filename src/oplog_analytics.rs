//! Knowledge coverage analytics and workspace reporting.
//!
//! Aggregates registry statistics to produce human-readable and
//! machine-consumable reports about the workspace knowledge state.

use serde::Serialize;
use std::collections::HashMap;

/// A single repo's knowledge snapshot.
#[derive(Debug, Serialize)]
pub struct RepoKnowledgeSnapshot {
    pub repo_id: String,
    pub symbol_count: i64,
    pub embedding_count: i64,
    pub call_count: i64,
    pub coverage_pct: f32, // embedding_count / symbol_count * 100
}

/// Workspace-wide knowledge report.
#[derive(Debug, Serialize)]
pub struct KnowledgeReport {
    pub repo_count: i64,
    pub total_symbols: i64,
    pub total_embeddings: i64,
    pub total_calls: i64,
    pub overall_coverage_pct: f32,
    pub repos: Vec<RepoKnowledgeSnapshot>,
    pub health_summary: HealthSummary,
    pub recent_activity: Vec<ActivityEvent>,
}

/// Health status aggregation across repos.
#[derive(Debug, Serialize)]
pub struct HealthSummary {
    pub dirty: i64,
    pub ahead: i64,
    pub behind: i64,
    pub diverged: i64,
    pub up_to_date: i64,
}

/// A recent activity event from OpLog.
#[derive(Debug, Serialize)]
pub struct ActivityEvent {
    pub repo_id: Option<String>,
    pub event_type: String,
    pub timestamp: String,
}

fn table_exists(conn: &rusqlite::Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        [name],
        |_| Ok(true),
    )
    .unwrap_or(false)
}

/// Generate a knowledge coverage report.
///
/// If `repo_id` is `Some`, the report focuses on that single repo.
/// If `None`, aggregates across the entire workspace.
pub fn generate_report(
    conn: &rusqlite::Connection,
    repo_id: Option<&str>,
    activity_limit: usize,
) -> anyhow::Result<KnowledgeReport> {
    let has_symbols = table_exists(conn, "code_symbols");
    let has_embeddings = table_exists(conn, "code_embeddings");
    let has_calls = table_exists(conn, "code_call_graph");

    let sym_sub = if has_symbols {
        "COALESCE((SELECT COUNT(*) FROM code_symbols cs WHERE cs.repo_id = r.id), 0)"
    } else {
        "0"
    };
    let emb_sub = if has_embeddings {
        "COALESCE((SELECT COUNT(*) FROM code_embeddings ce WHERE ce.repo_id = r.id), 0)"
    } else {
        "0"
    };
    let call_sub = if has_calls {
        "COALESCE((SELECT COUNT(*) FROM code_call_graph cc WHERE cc.repo_id = r.id), 0)"
    } else {
        "0"
    };

    // 1. Repo knowledge snapshots
    let mut repos: Vec<RepoKnowledgeSnapshot> = Vec::new();

    let sql = if let Some(rid) = repo_id {
        format!(
            "SELECT r.id,
                    {} as sym_count,
                    {} as emb_count,
                    {} as call_count
             FROM repos r
             WHERE r.id = '{}'",
            sym_sub, emb_sub, call_sub, rid
        )
    } else {
        format!(
            "SELECT r.id,
                    {} as sym_count,
                    {} as emb_count,
                    {} as call_count
             FROM repos r
             ORDER BY sym_count DESC",
            sym_sub, emb_sub, call_sub
        )
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;

    let mut total_symbols = 0i64;
    let mut total_embeddings = 0i64;
    let mut total_calls = 0i64;

    for row in rows {
        let (id, sym_count, emb_count, call_count) = row?;
        total_symbols += sym_count;
        total_embeddings += emb_count;
        total_calls += call_count;

        let coverage = if sym_count > 0 {
            (emb_count as f32 / sym_count as f32) * 100.0
        } else {
            0.0
        };

        repos.push(RepoKnowledgeSnapshot {
            repo_id: id,
            symbol_count: sym_count,
            embedding_count: emb_count,
            call_count,
            coverage_pct: coverage,
        });
    }

    let overall_coverage = if total_symbols > 0 {
        (total_embeddings as f32 / total_symbols as f32) * 100.0
    } else {
        0.0
    };

    // 2. Health summary (from repo_health_status view or raw query)
    // We approximate health by checking git status flags stored in repos or via a separate query.
    // For now, we count repos that have recent oplog events indicating issues.
    let health = if let Some(_rid) = repo_id {
        // Single-repo health: count recent oplog events by type
        let mut stmt = conn.prepare(
            "SELECT event_type, COUNT(*)
             FROM oplog
             WHERE repo_id = ?1
               AND timestamp >= datetime('now', '-7 days')
             GROUP BY event_type"
        )?;
        let rows = stmt.query_map([_rid], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut counts: HashMap<String, i64> = HashMap::new();
        for row in rows {
            let (et, count) = row?;
            counts.insert(et, count);
        }
        HealthSummary {
            dirty: counts.get("sync_preview_dirty").copied().unwrap_or(0),
            ahead: counts.get("sync_preview_ahead").copied().unwrap_or(0),
            behind: counts.get("sync_preview_behind").copied().unwrap_or(0),
            diverged: counts.get("sync_preview_diverged").copied().unwrap_or(0),
            up_to_date: counts.get("sync_preview_ok").copied().unwrap_or(0),
        }
    } else {
        // Workspace-wide: approximate from oplog latest health_check per repo
        // Note: oplog uses 'details' column (TEXT), not 'metadata'. We check details for status keywords.
        let mut stmt = conn.prepare(
            "SELECT 
                SUM(CASE WHEN details LIKE '%dirty%' THEN 1 ELSE 0 END) as dirty,
                SUM(CASE WHEN details LIKE '%ahead%' THEN 1 ELSE 0 END) as ahead,
                SUM(CASE WHEN details LIKE '%behind%' THEN 1 ELSE 0 END) as behind,
                SUM(CASE WHEN details LIKE '%diverged%' THEN 1 ELSE 0 END) as diverged,
                SUM(CASE WHEN details LIKE '%up_to_date%' OR details LIKE '%ok%' THEN 1 ELSE 0 END) as ok
             FROM (
                 SELECT repo_id, details,
                        ROW_NUMBER() OVER (PARTITION BY repo_id ORDER BY timestamp DESC) as rn
                 FROM oplog
                 WHERE event_type = 'health_check'
             )
             WHERE rn = 1"
        )?;
        let row = stmt.query_row([], |row| {
            Ok((
                row.get::<_, i64>(0).unwrap_or(0),
                row.get::<_, i64>(1).unwrap_or(0),
                row.get::<_, i64>(2).unwrap_or(0),
                row.get::<_, i64>(3).unwrap_or(0),
                row.get::<_, i64>(4).unwrap_or(0),
            ))
        })?;
        HealthSummary {
            dirty: row.0,
            ahead: row.1,
            behind: row.2,
            diverged: row.3,
            up_to_date: row.4,
        }
    };

    // 3. Recent activity
    let mut stmt = conn.prepare(
        "SELECT repo_id, event_type, timestamp
         FROM oplog
         ORDER BY timestamp DESC
         LIMIT ?"
    )?;
    let rows = stmt.query_map([activity_limit as i64], |row| {
        Ok(ActivityEvent {
            repo_id: row.get::<_, Option<String>>(0)?,
            event_type: row.get::<_, String>(1)?,
            timestamp: row.get::<_, String>(2)?,
        })
    })?;

    let mut recent_activity = Vec::new();
    for row in rows {
        recent_activity.push(row?);
    }

    Ok(KnowledgeReport {
        repo_count: repos.len() as i64,
        total_symbols,
        total_embeddings,
        total_calls,
        overall_coverage_pct: overall_coverage,
        repos,
        health_summary: health,
        recent_activity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_report_empty_db() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Create minimal schema
        conn.execute(
            "CREATE TABLE repos (id TEXT PRIMARY KEY, local_path TEXT NOT NULL, discovered_at TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE oplog (id INTEGER PRIMARY KEY AUTOINCREMENT, operation TEXT, repo_id TEXT, details TEXT, status TEXT, timestamp TEXT, event_type TEXT, duration_ms INTEGER, event_version INTEGER DEFAULT 1)",
            [],
        )
        .unwrap();

        let report = generate_report(&conn, None, 10).unwrap();
        assert_eq!(report.repo_count, 0);
        assert_eq!(report.total_symbols, 0);
        assert_eq!(report.overall_coverage_pct, 0.0);
    }

    #[test]
    fn test_generate_report_with_data() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE repos (id TEXT PRIMARY KEY, local_path TEXT NOT NULL, discovered_at TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE code_symbols (repo_id TEXT, file_path TEXT, symbol_type TEXT, name TEXT, line_start INTEGER, line_end INTEGER, signature TEXT, PRIMARY KEY(repo_id, file_path, name))",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE code_embeddings (repo_id TEXT, symbol_name TEXT, embedding BLOB, generated_at TEXT, PRIMARY KEY(repo_id, symbol_name))",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE oplog (id INTEGER PRIMARY KEY AUTOINCREMENT, operation TEXT, repo_id TEXT, details TEXT, status TEXT, timestamp TEXT, event_type TEXT, duration_ms INTEGER, event_version INTEGER DEFAULT 1)",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO repos VALUES ('repo1', '/path', '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO code_symbols VALUES ('repo1', 'a.rs', 'function', 'foo', 1, 2, 'fn foo()'),
                    ('repo1', 'a.rs', 'function', 'bar', 3, 4, 'fn bar()')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO code_embeddings VALUES ('repo1', 'foo', X'0000', '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO oplog (repo_id, event_type, timestamp) VALUES ('repo1', 'index', '2026-01-01')",
            [],
        )
        .unwrap();

        let report = generate_report(&conn, None, 10).unwrap();
        assert_eq!(report.repo_count, 1);
        assert_eq!(report.total_symbols, 2);
        assert_eq!(report.total_embeddings, 1);
        assert!((report.overall_coverage_pct - 50.0).abs() < 0.1);
        assert_eq!(report.repos[0].repo_id, "repo1");
        assert_eq!(report.repos[0].coverage_pct, 50.0);
        assert_eq!(report.recent_activity.len(), 1);
    }
}
