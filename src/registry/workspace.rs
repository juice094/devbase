use super::*;
use chrono::{DateTime, Utc};

pub fn save_workspace_snapshot(
    conn: &rusqlite::Connection,
    snapshot: &WorkspaceSnapshot,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO workspace_snapshots (repo_id, file_hash, checked_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![&snapshot.repo_id, &snapshot.file_hash, snapshot.checked_at.to_rfc3339()],
    )?;
    Ok(())
}

pub fn get_latest_workspace_snapshot(
    conn: &rusqlite::Connection,
    repo_id: &str,
) -> anyhow::Result<Option<WorkspaceSnapshot>> {
    let mut stmt = conn.prepare(
        "SELECT repo_id, file_hash, checked_at FROM workspace_snapshots WHERE repo_id = ?1",
    )?;
    let mut rows = stmt.query([repo_id])?;
    if let Some(row) = rows.next()? {
        let checked_at: String = row.get(2)?;
        let checked_at = DateTime::parse_from_rfc3339(&checked_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        Ok(Some(WorkspaceSnapshot {
            repo_id: row.get(0)?,
            file_hash: row.get(1)?,
            checked_at,
        }))
    } else {
        Ok(None)
    }
}

// ------------------------------------------------------------------
// Operation log
// ------------------------------------------------------------------
pub fn save_oplog(conn: &rusqlite::Connection, entry: &OplogEntry) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO oplog (operation, event_type, repo_id, details, status, timestamp, duration_ms, event_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            entry.event_type.as_str(),
            entry.event_type.as_str(),
            entry.repo_id.as_ref(),
            entry.details.as_ref(),
            &entry.status,
            entry.timestamp.to_rfc3339(),
            entry.duration_ms,
            entry.event_version
        ],
    )?;
    Ok(())
}

pub fn list_oplog(conn: &rusqlite::Connection, limit: i64) -> anyhow::Result<Vec<OplogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, event_type, repo_id, details, status, timestamp, duration_ms, event_version FROM oplog ORDER BY timestamp DESC LIMIT ?1"
    )?;
    let rows = stmt.query_map([limit], |row| {
        let ts: String = row.get(5)?;
        let timestamp = DateTime::parse_from_rfc3339(&ts)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let event_type_str: String = row.get(1)?;
        let event_type = event_type_str.parse().unwrap_or(OplogEventType::HealthCheck);
        Ok(OplogEntry {
            id: row.get(0)?,
            event_type,
            repo_id: row.get(2)?,
            details: row.get(3)?,
            status: row.get(4)?,
            timestamp,
            duration_ms: row.get(6)?,
            event_version: row.get(7).unwrap_or(0),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn list_oplog_by_repo(
    conn: &rusqlite::Connection,
    repo_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<OplogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, event_type, repo_id, details, status, timestamp, duration_ms, event_version FROM oplog WHERE repo_id = ?1 ORDER BY timestamp DESC LIMIT ?2"
    )?;
    let rows = stmt.query_map(rusqlite::params![repo_id, limit], |row| {
        let ts: String = row.get(5)?;
        let timestamp = DateTime::parse_from_rfc3339(&ts)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let event_type_str: String = row.get(1)?;
        let event_type = event_type_str.parse().unwrap_or(OplogEventType::HealthCheck);
        Ok(OplogEntry {
            id: row.get(0)?,
            event_type,
            repo_id: row.get(2)?,
            details: row.get(3)?,
            status: row.get(4)?,
            timestamp,
            duration_ms: row.get(6)?,
            event_version: row.get(7).unwrap_or(0),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_snapshot_roundtrip() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        let snapshot = WorkspaceSnapshot {
            repo_id: "repo-a".to_string(),
            file_hash: "abc123".to_string(),
            checked_at: Utc::now(),
        };
        save_workspace_snapshot(&conn, &snapshot).unwrap();
        let fetched = get_latest_workspace_snapshot(&conn, "repo-a").unwrap().unwrap();
        assert_eq!(fetched.repo_id, "repo-a");
        assert_eq!(fetched.file_hash, "abc123");
    }

    #[test]
    fn test_workspace_snapshot_missing() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let result = get_latest_workspace_snapshot(&conn, "missing").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_oplog_roundtrip() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let entry = OplogEntry {
            id: None,
            event_type: OplogEventType::HealthCheck,
            repo_id: Some("repo-a".to_string()),
            details: Some("details".to_string()),
            status: "ok".to_string(),
            timestamp: Utc::now(),
            duration_ms: Some(42),
            event_version: 1,
        };
        save_oplog(&conn, &entry).unwrap();
        let logs = list_oplog(&conn, 10).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].event_type, OplogEventType::HealthCheck);
        assert_eq!(logs[0].status, "ok");
        assert_eq!(logs[0].duration_ms, Some(42));
    }

    #[test]
    fn test_list_oplog_by_repo() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let e1 = OplogEntry {
            id: None,
            event_type: OplogEventType::Sync,
            repo_id: Some("repo-a".to_string()),
            details: None,
            status: "ok".to_string(),
            timestamp: Utc::now(),
            duration_ms: None,
            event_version: 1,
        };
        let e2 = OplogEntry {
            id: None,
            event_type: OplogEventType::HealthCheck,
            repo_id: Some("repo-b".to_string()),
            details: None,
            status: "ok".to_string(),
            timestamp: Utc::now(),
            duration_ms: None,
            event_version: 1,
        };
        save_oplog(&conn, &e1).unwrap();
        save_oplog(&conn, &e2).unwrap();

        let logs_a = list_oplog_by_repo(&conn, "repo-a", 10).unwrap();
        assert_eq!(logs_a.len(), 1);
        assert_eq!(logs_a[0].event_type, OplogEventType::Sync);

        let logs_b = list_oplog_by_repo(&conn, "repo-b", 10).unwrap();
        assert_eq!(logs_b.len(), 1);
        assert_eq!(logs_b[0].event_type, OplogEventType::HealthCheck);
    }

    #[test]
    fn test_list_oplog_limit() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        for i in 0..5 {
            let entry = OplogEntry {
                id: None,
                event_type: OplogEventType::Scan,
                repo_id: None,
                details: Some(format!("entry {}", i)),
                status: "ok".to_string(),
                timestamp: Utc::now(),
                duration_ms: None,
                event_version: 1,
            };
            save_oplog(&conn, &entry).unwrap();
        }
        let logs = list_oplog(&conn, 3).unwrap();
        assert_eq!(logs.len(), 3);
    }
}
