use super::*;
use chrono::{DateTime, Utc};

impl WorkspaceRegistry {
    pub fn save_vault_note(
        conn: &mut rusqlite::Connection,
        note: &crate::registry::VaultNote,
    ) -> anyhow::Result<()> {
        let tx = conn.transaction()?;
        // P1-1: filesystem-first — content/frontmatter no longer stored in SQLite.
        // The registry only keeps lightweight metadata (id, path, title, tags, links, updated_at).
        tx.execute(
            "INSERT OR REPLACE INTO vault_notes (id, path, title, frontmatter, tags, outgoing_links, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                &note.id,
                &note.path,
                note.title.as_ref(),
                note.frontmatter.as_ref(),
                note.tags.join(","),
                serde_json::to_string(&note.outgoing_links)?,
                note.created_at.to_rfc3339(),
                note.updated_at.to_rfc3339(),
            ],
        )?;
        // Sprint A-1: update vault_repo_links if linked_repo is specified
        if let Some(repo_id) = &note.linked_repo {
            tx.execute(
                "INSERT OR REPLACE INTO vault_repo_links (vault_id, repo_id) VALUES (?1, ?2)",
                rusqlite::params![&note.id, repo_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_vault_notes(
        conn: &rusqlite::Connection,
    ) -> anyhow::Result<Vec<crate::registry::VaultNote>> {
        let mut stmt = conn.prepare(
            "SELECT id, path, title, frontmatter, tags, outgoing_links, created_at, updated_at FROM vault_notes ORDER BY updated_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            let tags_raw: Option<String> = row.get(4)?;
            let links_raw: Option<String> = row.get(5)?;
            Ok(crate::registry::VaultNote {
                id: row.get(0)?,
                path: row.get(1)?,
                title: row.get(2)?,
                content: String::new(),
                frontmatter: row.get(3)?,
                tags: tags_raw
                    .map(|s| {
                        s.split(',')
                            .map(|t| t.trim().to_string())
                            .filter(|t| !t.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
                outgoing_links: links_raw
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
                linked_repo: None,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        let mut notes = Vec::new();
        for row in rows {
            notes.push(row?);
        }
        Ok(notes)
    }

    pub fn delete_vault_note(conn: &rusqlite::Connection, note_id: &str) -> anyhow::Result<()> {
        conn.execute("DELETE FROM vault_notes WHERE id = ?1", [note_id])?;
        conn.execute("DELETE FROM vault_repo_links WHERE vault_id = ?1", [note_id])?;
        Ok(())
    }
}
