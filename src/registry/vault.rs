use super::*;
use chrono::{DateTime, Utc};

impl WorkspaceRegistry {
    pub fn save_vault_note(
        conn: &mut rusqlite::Connection,
        note: &crate::registry::VaultNote,
    ) -> anyhow::Result<()> {
        let tx = conn.transaction()?;
        // Sprint A-1: update vault_repo_links if linked_repo is specified
        if let Some(repo_id) = &note.linked_repo {
            tx.execute(
                "INSERT OR REPLACE INTO vault_repo_links (vault_id, repo_id) VALUES (?1, ?2)",
                rusqlite::params![&note.id, repo_id],
            )?;
        }
        // Phase 2 Stage C: dual-write to entities table
        let metadata = serde_json::json!({
            "frontmatter": note.frontmatter,
            "tags": note.tags.join(","),
            "outgoing_links": note.outgoing_links,
            "linked_repo": note.linked_repo,
            "created_at": note.created_at.to_rfc3339(),
            "updated_at": note.updated_at.to_rfc3339(),
        });
        crate::registry::upsert_entity(
            &tx,
            &note.id,
            crate::registry::ENTITY_TYPE_VAULT_NOTE,
            note.title.as_deref().unwrap_or(&note.id),
            Some(&note.path),
            &metadata,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_vault_notes(
        conn: &rusqlite::Connection,
    ) -> anyhow::Result<Vec<crate::registry::VaultNote>> {
        let mut stmt = conn.prepare(
            "SELECT e.id, e.local_path, e.name, json_extract(e.metadata, '$.frontmatter'),
                    json_extract(e.metadata, '$.tags'), json_extract(e.metadata, '$.outgoing_links'),
                    json_extract(e.metadata, '$.linked_repo'),
                    json_extract(e.metadata, '$.created_at'), json_extract(e.metadata, '$.updated_at')
             FROM entities e
             WHERE e.entity_type = ?1
             ORDER BY json_extract(e.metadata, '$.updated_at') DESC"
        )?;
        let rows = stmt.query_map([crate::registry::ENTITY_TYPE_VAULT_NOTE], |row| {
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
                linked_repo: row.get(6)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
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

    pub fn get_vault_note(
        conn: &rusqlite::Connection,
        note_id: &str,
    ) -> anyhow::Result<Option<crate::registry::VaultNote>> {
        let mut stmt = conn.prepare(
            "SELECT e.id, e.local_path, e.name, json_extract(e.metadata, '$.frontmatter'),
                    json_extract(e.metadata, '$.tags'), json_extract(e.metadata, '$.outgoing_links'),
                    json_extract(e.metadata, '$.linked_repo'),
                    json_extract(e.metadata, '$.created_at'), json_extract(e.metadata, '$.updated_at')
             FROM entities e
             WHERE e.entity_type = ?1 AND e.id = ?2"
        )?;
        let mut rows = stmt.query_map(
            rusqlite::params![crate::registry::ENTITY_TYPE_VAULT_NOTE, note_id],
            |row| {
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
                    linked_repo: row.get(6)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            },
        )?;
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    pub fn delete_vault_note(conn: &rusqlite::Connection, note_id: &str) -> anyhow::Result<()> {
        conn.execute("DELETE FROM vault_repo_links WHERE vault_id = ?1", [note_id])?;
        // Phase 2 Stage C: keep entities in sync
        let _ = conn.execute("DELETE FROM entities WHERE id = ?1", [note_id]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::VaultNote;

    fn sample_note(id: &str) -> VaultNote {
        VaultNote {
            id: id.to_string(),
            path: format!("/tmp/{}.md", id),
            title: Some("Title".to_string()),
            content: "Content".to_string(),
            frontmatter: None,
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            outgoing_links: vec!["link1".to_string()],
            linked_repo: Some("repo-a".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_save_and_list_vault_note() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let note = sample_note("note1");
        WorkspaceRegistry::save_vault_note(&mut conn, &note).unwrap();

        let notes = WorkspaceRegistry::list_vault_notes(&conn).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "note1");
        assert_eq!(notes[0].tags, vec!["tag1", "tag2"]);
        assert_eq!(notes[0].outgoing_links, vec!["link1"]);
    }

    #[test]
    fn test_delete_vault_note() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let note = sample_note("note1");
        WorkspaceRegistry::save_vault_note(&mut conn, &note).unwrap();
        WorkspaceRegistry::delete_vault_note(&conn, "note1").unwrap();

        let notes = WorkspaceRegistry::list_vault_notes(&conn).unwrap();
        assert!(notes.is_empty());
    }

    #[test]
    fn test_list_vault_notes_empty() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let notes = WorkspaceRegistry::list_vault_notes(&conn).unwrap();
        assert!(notes.is_empty());
    }
}
