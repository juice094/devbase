//! Repository for repo health checks.

use crate::registry::HealthEntry;
use rusqlite::OptionalExtension;

pub struct HealthRepository<'a>(&'a rusqlite::Connection);

impl<'a> HealthRepository<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self(conn)
    }

    /// Get health status for a repo.
    pub fn get_health(&self, repo_id: &str) -> anyhow::Result<Option<HealthEntry>> {
        let mut stmt = self
            .0
            .prepare("SELECT status, ahead, behind, checked_at FROM repo_health WHERE repo_id = ?1")?;
        let result = stmt
            .query_row([repo_id], |row| {
                Ok(HealthEntry {
                    status: row.get(0)?,
                    ahead: row.get(1)?,
                    behind: row.get(2)?,
                    checked_at: row.get(3)?,
                })
            })
            .optional()?;
        Ok(result)
    }
}

impl<'a> super::Repository for HealthRepository<'a> {
    fn conn(&self) -> &rusqlite::Connection {
        self.0
    }
}
