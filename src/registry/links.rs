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
            "SELECT r.id, r.local_path FROM repos r
             JOIN vault_repo_links l ON r.id = l.repo_id
             WHERE l.vault_id = ?1
             ORDER BY r.id",
        )?;
        let rows = stmt.query_map([vault_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }
}
