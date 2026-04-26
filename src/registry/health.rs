use super::*;
use chrono::{DateTime, Utc};

impl WorkspaceRegistry {
    pub fn save_health(
        conn: &rusqlite::Connection,
        repo_id: &str,
        health: &HealthEntry,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO repo_health (repo_id, status, ahead, behind, checked_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                repo_id,
                &health.status,
                health.ahead as i64,
                health.behind as i64,
                health.checked_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn get_health(
        conn: &rusqlite::Connection,
        repo_id: &str,
    ) -> anyhow::Result<Option<HealthEntry>> {
        let mut stmt = conn.prepare(
            "SELECT status, ahead, behind, checked_at FROM repo_health WHERE repo_id = ?1",
        )?;
        let mut rows = stmt.query([repo_id])?;
        if let Some(row) = rows.next()? {
            let status: String = row.get(0)?;
            let ahead: i64 = row.get(1)?;
            let behind: i64 = row.get(2)?;
            let checked_at: String = row.get(3)?;
            let checked_at = match DateTime::parse_from_rfc3339(&checked_at) {
                Ok(dt) => dt.with_timezone(&Utc),
                Err(_) => return Ok(None),
            };
            Ok(Some(HealthEntry {
                status,
                ahead: ahead as usize,
                behind: behind as usize,
                checked_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn save_stars_cache(
        conn: &rusqlite::Connection,
        repo_id: &str,
        stars: u64,
    ) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO repo_stars_cache (repo_id, stars, fetched_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![repo_id, stars as i64, &now],
        )?;
        conn.execute(
            "INSERT INTO repo_stars_history (repo_id, stars, fetched_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![repo_id, stars as i64, &now],
        )?;
        conn.execute(
            "DELETE FROM repo_stars_history WHERE rowid NOT IN (
                SELECT rowid FROM repo_stars_history WHERE repo_id = ?1 ORDER BY fetched_at DESC LIMIT 30
            )",
            rusqlite::params![repo_id],
        )?;
        Ok(())
    }

    pub fn get_stars_cache(
        conn: &rusqlite::Connection,
        repo_id: &str,
    ) -> anyhow::Result<Option<(u64, DateTime<Utc>)>> {
        let mut stmt =
            conn.prepare("SELECT stars, fetched_at FROM repo_stars_cache WHERE repo_id = ?1")?;
        let mut rows = stmt.query([repo_id])?;
        if let Some(row) = rows.next()? {
            let stars: i64 = row.get(0)?;
            let fetched_at: String = row.get(1)?;
            let fetched_at = match DateTime::parse_from_rfc3339(&fetched_at) {
                Ok(dt) => dt.with_timezone(&Utc),
                Err(_) => return Ok(None),
            };
            Ok(Some((stars as u64, fetched_at)))
        } else {
            Ok(None)
        }
    }

    pub fn get_stars_history(
        conn: &rusqlite::Connection,
        repo_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<(u64, DateTime<Utc>)>> {
        let mut stmt = conn.prepare(
            "SELECT stars, fetched_at FROM repo_stars_history
             WHERE repo_id = ?1 ORDER BY fetched_at ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![repo_id, limit as i64], |row| {
            let stars: i64 = row.get(0)?;
            let fetched_at: String = row.get(1)?;
            Ok((
                stars as u64,
                DateTime::parse_from_rfc3339(&fetched_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_roundtrip() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        let health = HealthEntry {
            status: "healthy".to_string(),
            ahead: 2,
            behind: 1,
            checked_at: Utc::now(),
        };
        WorkspaceRegistry::save_health(&conn, "repo-a", &health).unwrap();
        let fetched = WorkspaceRegistry::get_health(&conn, "repo-a").unwrap().unwrap();
        assert_eq!(fetched.status, "healthy");
        assert_eq!(fetched.ahead, 2);
        assert_eq!(fetched.behind, 1);
    }

    #[test]
    fn test_stars_cache_roundtrip() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        WorkspaceRegistry::save_stars_cache(&conn, "repo-a", 42).unwrap();
        let (stars, _) = WorkspaceRegistry::get_stars_cache(&conn, "repo-a").unwrap().unwrap();
        assert_eq!(stars, 42);
    }

    #[test]
    fn test_stars_history() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        WorkspaceRegistry::save_stars_cache(&conn, "repo-a", 10).unwrap();
        WorkspaceRegistry::save_stars_cache(&conn, "repo-a", 20).unwrap();
        let history = WorkspaceRegistry::get_stars_history(&conn, "repo-a", 10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, 10);
        assert_eq!(history[1].0, 20);
    }
}
