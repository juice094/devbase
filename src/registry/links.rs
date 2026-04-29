use super::*;

impl WorkspaceRegistry {
    /// Get repo IDs linked to a vault note.
    pub fn get_linked_repos(
        conn: &rusqlite::Connection,
        vault_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        let mut stmt = conn
            .prepare("SELECT repo_id FROM vault_repo_links WHERE vault_id = ?1 ORDER BY repo_id")?;
        let rows = stmt.query_map([vault_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    /// Get vault note IDs linked to a repo.
    pub fn get_linked_vaults(
        conn: &rusqlite::Connection,
        repo_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        let mut stmt = conn.prepare(
            "SELECT vault_id FROM vault_repo_links WHERE repo_id = ?1 ORDER BY vault_id",
        )?;
        let rows = stmt.query_map([repo_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    /// Get vault notes (with title) linked to a repo.
    pub fn get_linked_vault_notes(
        conn: &rusqlite::Connection,
        repo_id: &str,
    ) -> anyhow::Result<Vec<(String, Option<String>)>> {
        let mut stmt = conn.prepare(
            "SELECT n.id, n.title FROM vault_notes n
             JOIN vault_repo_links l ON n.id = l.vault_id
             WHERE l.repo_id = ?1
             ORDER BY n.updated_at DESC",
        )?;
        let rows = stmt.query_map([repo_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    /// Get repos (with local_path) linked to a vault note.
    pub fn get_linked_repos_full(
        conn: &rusqlite::Connection,
        vault_id: &str,
    ) -> anyhow::Result<Vec<(String, String)>> {
        let mut stmt = conn.prepare(
            "SELECT e.id, e.local_path FROM entities e
             JOIN vault_repo_links l ON e.id = l.repo_id
             WHERE e.entity_type = 'repo' AND l.vault_id = ?1
             ORDER BY e.id",
        )?;
        let rows = stmt.query_map([vault_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use crate::registry::{VaultNote, WorkspaceRegistry};

    fn sample_note(id: &str, repo_id: Option<&str>) -> VaultNote {
        VaultNote {
            id: id.to_string(),
            path: format!("/tmp/{}.md", id),
            title: Some("Title".to_string()),
            content: "Content".to_string(),
            frontmatter: None,
            tags: vec![],
            outgoing_links: vec![],
            linked_repo: repo_id.map(|s| s.to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_get_linked_repos() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        let note = sample_note("note1", Some("repo-a"));
        WorkspaceRegistry::save_vault_note(&mut conn, &note).unwrap();

        let repos = WorkspaceRegistry::get_linked_repos(&conn, "note1").unwrap();
        assert_eq!(repos, vec!["repo-a"]);
    }

    #[test]
    fn test_get_linked_vaults() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        let note = sample_note("note1", Some("repo-a"));
        WorkspaceRegistry::save_vault_note(&mut conn, &note).unwrap();

        let vaults = WorkspaceRegistry::get_linked_vaults(&conn, "repo-a").unwrap();
        assert_eq!(vaults, vec!["note1"]);
    }

    #[test]
    fn test_get_linked_vault_notes() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        let note = sample_note("note1", Some("repo-a"));
        WorkspaceRegistry::save_vault_note(&mut conn, &note).unwrap();

        let notes = WorkspaceRegistry::get_linked_vault_notes(&conn, "repo-a").unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].0, "note1");
        assert_eq!(notes[0].1, Some("Title".to_string()));
    }

    #[test]
    fn test_get_linked_repos_full() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        let note = sample_note("note1", Some("repo-a"));
        WorkspaceRegistry::save_vault_note(&mut conn, &note).unwrap();

        let repos = WorkspaceRegistry::get_linked_repos_full(&conn, "note1").unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].0, "repo-a");
    }

    #[test]
    fn test_get_linked_repos_empty() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let repos = WorkspaceRegistry::get_linked_repos(&conn, "note-none").unwrap();
        assert!(repos.is_empty());
    }
}
