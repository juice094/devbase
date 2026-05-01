//! Repository for repo entities.

use crate::registry::{RepoEntry, RemoteEntry};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

pub struct RepoRepository<'a>(&'a rusqlite::Connection);

impl<'a> RepoRepository<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self(conn)
    }

    /// List all repos, optionally filtered by a substring on id/local_path.
    pub fn list_repos(&self, filter: Option<&str>) -> anyhow::Result<Vec<RepoEntry>> {
        // TODO: migrate from registry::repo::list_repos
        todo!()
    }

    /// Get a single repo by ID.
    pub fn get_repo(&self, repo_id: &str) -> anyhow::Result<Option<RepoEntry>> {
        // TODO: migrate from registry::repo::get_repo
        todo!()
    }

    /// Save or update a repo.
    pub fn save_repo(&self, repo: &RepoEntry) -> anyhow::Result<()> {
        // TODO: migrate from registry::repo::save_repo
        todo!()
    }

    /// Update repo language.
    pub fn update_language(&self, repo_id: &str, language: Option<&str>) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.0.execute(
            "UPDATE entities SET language = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![language, &now, repo_id],
        )?;
        Ok(())
    }

    /// Update repo tier (data_tier).
    pub fn update_tier(&self, repo_id: &str, tier: &str) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.0.execute(
            "UPDATE entities SET data_tier = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![tier, &now, repo_id],
        )?;
        Ok(())
    }

    /// Update repo workspace_type.
    pub fn update_workspace_type(&self, repo_id: &str, workspace_type: &str) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.0.execute(
            "UPDATE entities SET workspace_type = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![workspace_type, &now, repo_id],
        )?;
        Ok(())
    }

    /// Update last_synced_at.
    pub fn update_last_synced_at(
        &self,
        repo_id: &str,
        timestamp: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.0.execute(
            "UPDATE entities SET last_synced_at = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![timestamp.to_rfc3339(), &now, repo_id],
        )?;
        Ok(())
    }

    /// Get the origin remote upstream_url for a repo.
    pub fn get_origin_remote(&self, repo_id: &str) -> anyhow::Result<Option<String>> {
        let mut stmt = self.0.prepare(
            "SELECT upstream_url FROM repo_remotes WHERE repo_id = ?1 AND remote_name = 'origin'",
        )?;
        let url: Option<String> = stmt.query_row([repo_id], |row| row.get(0)).optional()?;
        Ok(url)
    }
}

impl<'a> super::Repository for RepoRepository<'a> {
    fn conn(&self) -> &rusqlite::Connection {
        self.0
    }
}
