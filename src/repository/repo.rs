//! Repository for repo entities.

use crate::registry::{RepoEntry, RemoteEntry, ENTITY_TYPE_REPO};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use std::path::PathBuf;

pub struct RepoRepository<'a>(&'a rusqlite::Connection);

impl<'a> RepoRepository<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self(conn)
    }

    /// List all repos, optionally filtered by a substring on id/local_path.
    pub fn list_repos(&self, filter: Option<&str>) -> anyhow::Result<Vec<RepoEntry>> {
        let filter_clause = if filter.is_some() {
            "AND (e.id LIKE ?1 OR e.local_path LIKE ?1)"
        } else {
            ""
        };
        let sql = format!(
            "SELECT e.id, e.local_path, (SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = e.id) as tags,
                    e.language, e.discovered_at,
                    e.workspace_type, e.data_tier,
                    e.last_synced_at, e.stars,
                    rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
             FROM entities e
             LEFT JOIN repo_remotes rm ON e.id = rm.repo_id
             WHERE e.entity_type = '{}' {}
             ORDER BY e.id, rm.remote_name",
            ENTITY_TYPE_REPO,
            filter_clause
        );
        let stmt = self.0.prepare(&sql)?;
        let params: &[&dyn rusqlite::ToSql] = match filter {
            Some(f) => &[&format!("%{}%", f)],
            None => &[],
        };
        collect_repos_from_stmt(stmt, params)
    }

    /// Get a single repo by ID.
    pub fn get_repo(&self, repo_id: &str) -> anyhow::Result<Option<RepoEntry>> {
        let sql = format!(
            "SELECT e.id, e.local_path, (SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = e.id) as tags,
                    e.language, e.discovered_at,
                    e.workspace_type, e.data_tier,
                    e.last_synced_at, e.stars,
                    rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
             FROM entities e
             LEFT JOIN repo_remotes rm ON e.id = rm.repo_id
             WHERE e.entity_type = '{}' AND e.id = ?1
             ORDER BY rm.remote_name",
            ENTITY_TYPE_REPO
        );
        let stmt = self.0.prepare(&sql)?;
        let entries = collect_repos_from_stmt(stmt, &[&repo_id])?;
        Ok(entries.into_iter().next())
    }

    /// Save or update a repo.
    pub fn save_repo(&self, repo: &RepoEntry) -> anyhow::Result<()> {
        let tx = self.0.unchecked_transaction()?;
        let now = chrono::Utc::now().to_rfc3339();
        tx.execute(
            &format!(
                "INSERT INTO entities (
                    id, entity_type, name, source_url, local_path, metadata,
                    content_hash, created_at, updated_at,
                    language, discovered_at, workspace_type, data_tier, last_synced_at, stars
                ) VALUES (
                    ?1, '{}', ?2, NULL, ?3, ?4, NULL, ?5, ?5,
                    ?6, ?7, ?8, ?9, ?10, ?11
                )
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    local_path = excluded.local_path,
                    metadata = excluded.metadata,
                    updated_at = excluded.updated_at,
                    language = excluded.language,
                    discovered_at = excluded.discovered_at,
                    workspace_type = excluded.workspace_type,
                    data_tier = excluded.data_tier,
                    last_synced_at = excluded.last_synced_at,
                    stars = excluded.stars",
                ENTITY_TYPE_REPO
            ),
            rusqlite::params![
                &repo.id,
                &repo.id,
                repo.local_path.to_str(),
                serde_json::json!({"tags": repo.tags.join(",")}).to_string(),
                &now,
                repo.language.as_deref(),
                repo.discovered_at.to_rfc3339(),
                &repo.workspace_type,
                &repo.data_tier,
                repo.last_synced_at.map(|dt| dt.to_rfc3339()),
                repo.stars.map(|s| s as i64),
            ],
        )?;
        tx.execute("DELETE FROM repo_tags WHERE repo_id = ?1", [&repo.id])?;
        for tag in &repo.tags {
            tx.execute(
                "INSERT OR REPLACE INTO repo_tags (repo_id, tag) VALUES (?1, ?2)",
                rusqlite::params![&repo.id, tag],
            )?;
        }
        tx.execute("DELETE FROM repo_remotes WHERE repo_id = ?1", [&repo.id])?;
        for remote in &repo.remotes {
            tx.execute(
                "INSERT INTO repo_remotes (repo_id, remote_name, upstream_url, default_branch, last_sync) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    &repo.id,
                    &remote.remote_name,
                    remote.upstream_url.as_ref(),
                    remote.default_branch.as_ref(),
                    remote.last_sync.map(|dt| dt.to_rfc3339())
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
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

/// Helper to collect repo rows (with potential remotes) into RepoEntry structs.
fn collect_repos_from_stmt(
    mut stmt: rusqlite::Statement<'_>,
    params: &[&dyn rusqlite::ToSql],
) -> anyhow::Result<Vec<RepoEntry>> {
    let rows = stmt.query_map(params, |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<i64>>(8)?,
            row.get::<_, Option<String>>(9)?,
            row.get::<_, Option<String>>(10)?,
            row.get::<_, Option<String>>(11)?,
            row.get::<_, Option<String>>(12)?,
        ))
    })?;
    let mut entries = Vec::new();
    for row in rows {
        let (
            id,
            local_path,
            tags,
            language,
            discovered_at,
            workspace_type,
            data_tier,
            last_synced_at,
            stars,
            remote_name,
            upstream_url,
            default_branch,
            last_sync,
        ) = row?;
        let local_path = PathBuf::from(local_path);
        let discovered_at = discovered_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        let tags: Vec<String> = tags
            .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
            .unwrap_or_default();
        let workspace_type = workspace_type.unwrap_or_else(|| "git".to_string());
        let data_tier = data_tier.unwrap_or_else(|| "private".to_string());
        let last_synced_at = last_synced_at
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc)));
        let stars = stars.map(|s| s as u64);
        let remote = remote_name.map(|name| RemoteEntry {
            remote_name: name,
            upstream_url,
            default_branch,
            last_sync: last_sync.and_then(|s| {
                DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))
            }),
        });
        if let Some(entry) = entries.last_mut().filter(|e: &&mut RepoEntry| e.id == id) {
            if let Some(r) = remote {
                entry.remotes.push(r);
            }
        } else {
            let mut remotes = Vec::new();
            if let Some(r) = remote {
                remotes.push(r);
            }
            entries.push(RepoEntry {
                id,
                local_path,
                tags,
                language,
                workspace_type,
                data_tier,
                last_synced_at,
                stars,
                discovered_at,
                remotes,
            });
        }
    }
    Ok(entries)
}
