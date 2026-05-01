use super::*;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

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

pub fn list_repos(conn: &rusqlite::Connection) -> anyhow::Result<Vec<RepoEntry>> {
    let stmt = conn.prepare(&format!(
        "SELECT e.id, e.local_path, (SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = e.id) as tags,
                json_extract(e.metadata, '$.language'), json_extract(e.metadata, '$.discovered_at'),
                json_extract(e.metadata, '$.workspace_type'), json_extract(e.metadata, '$.data_tier'),
                json_extract(e.metadata, '$.last_synced_at'), json_extract(e.metadata, '$.stars'),
                rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
         FROM entities e
         LEFT JOIN repo_remotes rm ON e.id = rm.repo_id
         WHERE e.entity_type = '{}'
         ORDER BY e.id, rm.remote_name",
        super::ENTITY_TYPE_REPO
    ))?;
    collect_repos_from_stmt(stmt, &[])
}

pub fn list_repos_stale_health(
    conn: &rusqlite::Connection,
    threshold: &str,
) -> anyhow::Result<Vec<RepoEntry>> {
    let stmt = conn.prepare(&format!(
        "SELECT e.id, e.local_path, (SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = e.id) as tags,
                json_extract(e.metadata, '$.language'), json_extract(e.metadata, '$.discovered_at'),
                json_extract(e.metadata, '$.workspace_type'), json_extract(e.metadata, '$.data_tier'),
                json_extract(e.metadata, '$.last_synced_at'), json_extract(e.metadata, '$.stars'),
                rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
         FROM entities e
         LEFT JOIN repo_remotes rm ON e.id = rm.repo_id
         WHERE e.entity_type = '{}' AND (
             NOT EXISTS (SELECT 1 FROM repo_health h WHERE h.repo_id = e.id)
             OR EXISTS (SELECT 1 FROM repo_health h WHERE h.repo_id = e.id AND h.checked_at < ?1)
         )
         ORDER BY e.id, rm.remote_name",
        super::ENTITY_TYPE_REPO
    ))?;
    collect_repos_from_stmt(stmt, &[&threshold])
}

pub fn list_repos_need_index(
    conn: &rusqlite::Connection,
    threshold: &str,
) -> anyhow::Result<Vec<RepoEntry>> {
    let stmt = conn.prepare(&format!(
        "SELECT e.id, e.local_path, (SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = e.id) as tags,
                json_extract(e.metadata, '$.language'), json_extract(e.metadata, '$.discovered_at'),
                json_extract(e.metadata, '$.workspace_type'), json_extract(e.metadata, '$.data_tier'),
                json_extract(e.metadata, '$.last_synced_at'), json_extract(e.metadata, '$.stars'),
                rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
         FROM entities e
         LEFT JOIN repo_remotes rm ON e.id = rm.repo_id
         WHERE e.entity_type = '{}' AND (
             NOT EXISTS (SELECT 1 FROM repo_summaries s WHERE s.repo_id = e.id)
             OR EXISTS (SELECT 1 FROM repo_summaries s WHERE s.repo_id = e.id AND s.generated_at < ?1)
             OR json_extract(e.metadata, '$.language') IS NULL
         )
         ORDER BY e.id, rm.remote_name",
        super::ENTITY_TYPE_REPO
    ))?;
    collect_repos_from_stmt(stmt, &[&threshold])
}

pub fn save_repo(conn: &mut rusqlite::Connection, repo: &RepoEntry) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    // Entities is the single source of truth for repo metadata.
    upsert_entity_for_repo(&tx, repo)?;
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

pub fn update_repo_language(
    conn: &rusqlite::Connection,
    repo_id: &str,
    language: Option<&str>,
) -> anyhow::Result<()> {
    let tx = conn.unchecked_transaction()?;
    super::entity::update_entity_metadata_field(
        &tx,
        repo_id,
        "language",
        language.unwrap_or("null"),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn update_repo_tier(
    conn: &rusqlite::Connection,
    repo_id: &str,
    tier: &str,
) -> anyhow::Result<()> {
    let tx = conn.unchecked_transaction()?;
    super::entity::update_entity_metadata_field(&tx, repo_id, "data_tier", tier)?;
    tx.commit()?;
    Ok(())
}

pub fn update_repo_workspace_type(
    conn: &rusqlite::Connection,
    repo_id: &str,
    workspace_type: &str,
) -> anyhow::Result<()> {
    let tx = conn.unchecked_transaction()?;
    super::entity::update_entity_metadata_field(&tx, repo_id, "workspace_type", workspace_type)?;
    tx.commit()?;
    Ok(())
}

#[allow(dead_code)]
pub fn update_repo_last_synced_at(
    conn: &rusqlite::Connection,
    repo_id: &str,
    timestamp: DateTime<Utc>,
) -> anyhow::Result<()> {
    let tx = conn.unchecked_transaction()?;
    super::entity::update_entity_metadata_field(
        &tx,
        repo_id,
        "last_synced_at",
        &timestamp.to_rfc3339(),
    )?;
    tx.commit()?;
    Ok(())
}

#[allow(dead_code)]
pub fn list_workspaces_by_tier(
    conn: &rusqlite::Connection,
    tier: &str,
) -> anyhow::Result<Vec<RepoEntry>> {
    let stmt = conn.prepare(&format!(
        "SELECT e.id, e.local_path, (SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = e.id) as tags,
                json_extract(e.metadata, '$.language'), json_extract(e.metadata, '$.discovered_at'),
                json_extract(e.metadata, '$.workspace_type'), json_extract(e.metadata, '$.data_tier'),
                json_extract(e.metadata, '$.last_synced_at'), json_extract(e.metadata, '$.stars'),
                rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
         FROM entities e
         LEFT JOIN repo_remotes rm ON e.id = rm.repo_id
         WHERE e.entity_type = '{}' AND json_extract(e.metadata, '$.data_tier') = ?1
         ORDER BY e.id, rm.remote_name",
        super::ENTITY_TYPE_REPO
    ))?;
    collect_repos_from_stmt(stmt, &[&tier])
}

/// Sync repo_tags sub-table back into entities.metadata.tags.
pub fn sync_repo_tags_to_entity(conn: &rusqlite::Connection, repo_id: &str) -> anyhow::Result<()> {
    let tags: Option<String> = conn
        .query_row(
            "SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = ?1",
            [repo_id],
            |row| row.get(0),
        )
        .unwrap_or(None);
    super::entity::update_entity_metadata_field(
        conn,
        repo_id,
        "tags",
        &serde_json::to_string(&tags).unwrap_or_else(|_| "null".to_string()),
    )?;
    Ok(())
}

/// Dual-write helper: upsert a repo into the unified entities table.
/// Entities is first-class; this writes directly from the RepoEntry without reading repos.
fn upsert_entity_for_repo(conn: &rusqlite::Connection, repo: &RepoEntry) -> anyhow::Result<()> {
    let metadata = serde_json::json!({
        "language": repo.language,
        "discovered_at": repo.discovered_at.to_rfc3339(),
        "workspace_type": repo.workspace_type,
        "data_tier": repo.data_tier,
        "stars": repo.stars.map(|s| s as i64),
        "last_synced_at": repo.last_synced_at.map(|dt| dt.to_rfc3339()),
        "tags": repo.tags.join(","),
    });
    super::upsert_entity(
        conn,
        &repo.id,
        super::ENTITY_TYPE_REPO,
        &repo.id,
        Some(&repo.local_path.to_string_lossy()),
        &metadata,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::RemoteEntry;

    fn sample_repo(id: &str, path: &str) -> RepoEntry {
        RepoEntry {
            id: id.to_string(),
            local_path: PathBuf::from(path),
            tags: vec![],
            discovered_at: Utc::now(),
            language: Some("rust".to_string()),
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        }
    }

    #[test]
    fn test_list_repos_empty() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let repos = list_repos(&conn).unwrap();
        assert!(repos.is_empty());
    }

    #[test]
    fn test_save_and_list_repo() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let repo = sample_repo("repo1", "/tmp/repo1");
        save_repo(&mut conn, &repo).unwrap();

        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, "repo1");
        assert_eq!(repos[0].local_path, PathBuf::from("/tmp/repo1"));
        assert_eq!(repos[0].language, Some("rust".to_string()));
        assert_eq!(repos[0].workspace_type, "git");
        assert_eq!(repos[0].data_tier, "private");
    }

    #[test]
    fn test_save_repo_with_tags() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let mut repo = sample_repo("repo1", "/tmp/repo1");
        repo.tags = vec!["cli".to_string(), "rust".to_string()];
        save_repo(&mut conn, &repo).unwrap();

        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos[0].tags.len(), 2);
        assert!(repos[0].tags.contains(&"cli".to_string()));
        assert!(repos[0].tags.contains(&"rust".to_string()));
    }

    #[test]
    fn test_save_repo_with_remotes() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let mut repo = sample_repo("repo1", "/tmp/repo1");
        repo.remotes.push(RemoteEntry {
            remote_name: "origin".to_string(),
            upstream_url: Some("https://github.com/user/repo1".to_string()),
            default_branch: Some("main".to_string()),
            last_sync: None,
        });
        save_repo(&mut conn, &repo).unwrap();

        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos[0].remotes.len(), 1);
        assert_eq!(repos[0].remotes[0].remote_name, "origin");
        assert_eq!(
            repos[0].remotes[0].upstream_url,
            Some("https://github.com/user/repo1".to_string())
        );
    }

    #[test]
    fn test_save_repo_updates_existing() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let mut repo = sample_repo("repo1", "/tmp/repo1");
        repo.tags = vec!["a".to_string()];
        save_repo(&mut conn, &repo).unwrap();

        let mut repo2 = sample_repo("repo1", "/tmp/repo1_moved");
        repo2.tags = vec!["b".to_string(), "c".to_string()];
        repo2.language = Some("go".to_string());
        save_repo(&mut conn, &repo2).unwrap();

        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].local_path, PathBuf::from("/tmp/repo1_moved"));
        assert_eq!(repos[0].language, Some("go".to_string()));
        assert_eq!(repos[0].tags.len(), 2);
        assert!(!repos[0].tags.contains(&"a".to_string()));
    }

    #[test]
    fn test_list_workspaces_by_tier() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let mut private = sample_repo("private_repo", "/tmp/p");
        private.data_tier = "private".to_string();

        let mut public = sample_repo("public_repo", "/tmp/pub");
        public.data_tier = "public".to_string();

        save_repo(&mut conn, &private).unwrap();
        save_repo(&mut conn, &public).unwrap();

        let private_repos = list_workspaces_by_tier(&conn, "private").unwrap();
        assert_eq!(private_repos.len(), 1);
        assert_eq!(private_repos[0].id, "private_repo");

        let public_repos = list_workspaces_by_tier(&conn, "public").unwrap();
        assert_eq!(public_repos.len(), 1);
        assert_eq!(public_repos[0].id, "public_repo");

        let none = list_workspaces_by_tier(&conn, "nonexistent").unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn test_save_repo_with_stars() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let mut repo = sample_repo("starred", "/tmp/s");
        repo.stars = Some(100);
        save_repo(&mut conn, &repo).unwrap();

        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos[0].stars, Some(100));
    }

    #[test]
    fn test_update_repo_language() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let repo = sample_repo("repo1", "/tmp/repo1");
        save_repo(&mut conn, &repo).unwrap();

        update_repo_language(&conn, "repo1", Some("go")).unwrap();
        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos[0].language, Some("go".to_string()));
    }

    #[test]
    fn test_update_repo_tier() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let repo = sample_repo("repo1", "/tmp/repo1");
        save_repo(&mut conn, &repo).unwrap();

        update_repo_tier(&conn, "repo1", "public").unwrap();
        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos[0].data_tier, "public");
    }

    #[test]
    fn test_update_repo_workspace_type() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let repo = sample_repo("repo1", "/tmp/repo1");
        save_repo(&mut conn, &repo).unwrap();

        update_repo_workspace_type(&conn, "repo1", "openclaw").unwrap();
        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos[0].workspace_type, "openclaw");
    }

    #[test]
    fn test_update_repo_last_synced_at() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let repo = sample_repo("repo1", "/tmp/repo1");
        save_repo(&mut conn, &repo).unwrap();

        let now = chrono::Utc::now();
        update_repo_last_synced_at(&conn, "repo1", now).unwrap();
        let repos = list_repos(&conn).unwrap();
        assert!(repos[0].last_synced_at.is_some());
    }

    #[test]
    fn test_list_repos_stale_health() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();

        // No health record → should be stale
        let now = chrono::Utc::now().to_rfc3339();
        let stale = list_repos_stale_health(&conn, &now).unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].id, "repo-a");

        // Save health with current timestamp
        let health = crate::registry::HealthEntry {
            status: "healthy".to_string(),
            ahead: 0,
            behind: 0,
            checked_at: chrono::Utc::now(),
        };
        crate::registry::health::save_health(&conn, "repo-a", &health).unwrap();

        // With current threshold → checked_at is not earlier than threshold → not stale
        let stale = list_repos_stale_health(&conn, &now).unwrap();
        assert!(stale.is_empty());

        // With future threshold → checked_at < future → stale again
        let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        let stale = list_repos_stale_health(&conn, &future).unwrap();
        assert_eq!(stale.len(), 1);
    }

    #[test]
    fn test_list_repos_need_index() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();

        // No summary → should need index
        let now = chrono::Utc::now().to_rfc3339();
        let need = list_repos_need_index(&conn, &now).unwrap();
        assert_eq!(need.len(), 1);
        assert_eq!(need[0].id, "repo-a");

        // Save summary with current timestamp
        crate::registry::knowledge::save_summary(&conn, "repo-a", "A test summary", "test")
            .unwrap();

        // With current threshold → generated_at is not earlier → not need index
        let need = list_repos_need_index(&conn, &now).unwrap();
        assert!(need.is_empty());

        // With future threshold → generated_at < future → need index again
        let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        let need = list_repos_need_index(&conn, &future).unwrap();
        assert_eq!(need.len(), 1);
    }
}
