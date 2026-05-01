//! Repository for workspace snapshot operations.

use crate::registry::WorkspaceSnapshot;
use rusqlite::OptionalExtension;

pub struct WorkspaceRepository<'a>(&'a rusqlite::Connection);

impl<'a> WorkspaceRepository<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self(conn)
    }

    /// Get the latest workspace snapshot for a repo.
    pub fn get_latest_snapshot(&self, repo_id: &str) -> anyhow::Result<Option<WorkspaceSnapshot>> {
        let mut stmt = self.0.prepare(
            "SELECT repo_id, file_hash, checked_at FROM workspace_snapshots WHERE repo_id = ?1",
        )?;
        let result = stmt
            .query_row([repo_id], |row| {
                Ok(WorkspaceSnapshot {
                    repo_id: row.get(0)?,
                    file_hash: row.get(1)?,
                    checked_at: row.get(2)?,
                })
            })
            .optional()?;
        Ok(result)
    }
}

impl<'a> super::Repository for WorkspaceRepository<'a> {
    fn conn(&self) -> &rusqlite::Connection {
        self.0
    }
}
