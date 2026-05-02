use chrono::Utc;

// Entity type constants for the unified entities table.
pub const ENTITY_TYPE_REPO: &str = "repo";
pub const ENTITY_TYPE_SKILL: &str = "skill";
pub const ENTITY_TYPE_PAPER: &str = "paper";
pub const ENTITY_TYPE_VAULT_NOTE: &str = "vault_note";
pub const ENTITY_TYPE_WORKFLOW: &str = "workflow";

/// Upsert a generic row into the `entities` table.
/// `local_path` may be `None` for entities that have no filesystem presence.
pub fn upsert_entity(
    conn: &rusqlite::Connection,
    id: &str,
    entity_type: &str,
    name: &str,
    local_path: Option<&str>,
    metadata: &serde_json::Value,
) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        &format!(
            "INSERT INTO entities (id, entity_type, name, source_url, local_path, metadata, created_at, updated_at)
             VALUES (?1, '{}', ?2, NULL, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                local_path = excluded.local_path,
                metadata = excluded.metadata,
                updated_at = excluded.updated_at",
            entity_type
        ),
        rusqlite::params![id, name, local_path, metadata.to_string(), &now],
    )?;
    Ok(())
}

/// Check whether an entity with the given ID exists.
pub fn entity_exists(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<bool> {
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM entities WHERE id = ?1", [id], |row| row.get(0))?;
    Ok(count > 0)
}

/// Delete an entity by ID.
pub fn delete_entity(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM entities WHERE id = ?1", [id])?;
    Ok(())
}

/// Update a single JSON field in entities.metadata for an entity.
/// When `value` is the JSON literal `"null"`, the key is removed instead.
pub fn update_entity_metadata_field(
    conn: &rusqlite::Connection,
    entity_id: &str,
    field: &str,
    value: &str,
) -> anyhow::Result<()> {
    if value == "null" {
        conn.execute(
            &format!(
                "UPDATE entities SET metadata = json_remove(metadata, '$.{field}'), updated_at = ?1 WHERE id = ?2"
            ),
            rusqlite::params![Utc::now().to_rfc3339(), entity_id],
        )?;
    } else {
        conn.execute(
            &format!(
                "UPDATE entities SET metadata = json_set(metadata, '$.{field}', ?1), updated_at = ?2 WHERE id = ?3"
            ),
            rusqlite::params![value, Utc::now().to_rfc3339(), entity_id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE entities (
                id TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                name TEXT NOT NULL,
                source_url TEXT,
                local_path TEXT,
                metadata TEXT,
                content_hash TEXT,
                created_at TEXT,
                updated_at TEXT,
                language TEXT,
                discovered_at TEXT,
                workspace_type TEXT DEFAULT 'git',
                data_tier TEXT DEFAULT 'private',
                last_synced_at TEXT,
                stars INTEGER
            )",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_entity_crud() {
        let conn = in_memory();

        // Upsert
        upsert_entity(
            &conn,
            "ent-1",
            "repo",
            "test-repo",
            Some("/tmp/test"),
            &serde_json::json!({"lang": "rust"}),
        )
        .unwrap();
        assert!(entity_exists(&conn, "ent-1").unwrap());

        // Update metadata
        update_entity_metadata_field(&conn, "ent-1", "lang", "\"go\"").unwrap();

        // Delete
        delete_entity(&conn, "ent-1").unwrap();
        assert!(!entity_exists(&conn, "ent-1").unwrap());
    }

    #[test]
    fn test_entity_metadata_remove_null() {
        let conn = in_memory();
        upsert_entity(
            &conn,
            "ent-2",
            "skill",
            "test-skill",
            None,
            &serde_json::json!({"key": "val"}),
        )
        .unwrap();

        update_entity_metadata_field(&conn, "ent-2", "key", "null").unwrap();
        // Should not panic; verify entity still exists
        assert!(entity_exists(&conn, "ent-2").unwrap());
    }
}
