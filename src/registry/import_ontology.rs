use std::path::Path;

use rusqlite::Connection;

type Result<T> = std::result::Result<T, anyhow::Error>;

/// Statistics from an ontology import run.
#[derive(Debug, Clone, Default)]
pub struct OntologyImportStats {
    pub entities_added: usize,
    pub entities_updated: usize,
    pub relations_added: usize,
    pub relations_updated: usize,
    pub errors: Vec<String>,
}

/// Import ontology entities and relations from an OpenClaw-compatible workspace.
pub fn import_ontology(conn: &Connection, workspace_path: &Path) -> Result<OntologyImportStats> {
    let entities_dir = workspace_path.join("ontology").join("entities");
    let relations_file =
        workspace_path.join("ontology").join("relations").join("core-relations.jsonl");

    let mut stats = OntologyImportStats::default();

    // Phase 1: Import entities
    if entities_dir.is_dir() {
        for entry in std::fs::read_dir(&entities_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                match import_entity_file(conn, &path) {
                    Ok((added, updated)) => {
                        stats.entities_added += added;
                        stats.entities_updated += updated;
                    }
                    Err(e) => {
                        stats.errors.push(format!("{}: {}", path.display(), e));
                    }
                }
            }
        }
    }

    // Phase 2: Import relations
    if relations_file.exists() {
        match import_relations_file(conn, &relations_file) {
            Ok((added, updated)) => {
                stats.relations_added += added;
                stats.relations_updated += updated;
            }
            Err(e) => {
                stats.errors.push(format!("{}: {}", relations_file.display(), e));
            }
        }
    }

    Ok(stats)
}

fn import_entity_file(conn: &Connection, path: &Path) -> Result<(usize, usize)> {
    let content = std::fs::read_to_string(path)?;
    let entity: serde_json::Value = serde_json::from_str(&content)?;

    let entity_id = entity["entity_id"].as_str().unwrap_or("unknown");
    let entity_type = entity["type"].as_str().unwrap_or("ontology_node");
    let name = entity["name"].as_str().unwrap_or(entity_id);

    // Ensure entity type exists
    conn.execute(
        "INSERT OR IGNORE INTO entity_types (name, schema_json, description, created_at) VALUES (?1, '{}', ?2, datetime('now'))",
        rusqlite::params![entity_type, format!("Ontology entity type: {}", entity_type)],
    )?;

    let metadata = serde_json::to_string(&entity).unwrap_or_default();
    let now = chrono::Utc::now().to_rfc3339();
    let existing: Option<String> = conn
        .query_row("SELECT id FROM entities WHERE id = ?1", rusqlite::params![entity_id], |row| {
            row.get(0)
        })
        .ok();

    conn.execute(
        "INSERT INTO entities (id, entity_type, name, metadata, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(id) DO UPDATE SET
             entity_type = excluded.entity_type,
             name = excluded.name,
             metadata = excluded.metadata,
             updated_at = excluded.updated_at",
        rusqlite::params![entity_id, entity_type, name, metadata, now],
    )?;

    if existing.is_some() {
        Ok((0, 1))
    } else {
        Ok((1, 0))
    }
}

fn import_relations_file(conn: &Connection, path: &Path) -> Result<(usize, usize)> {
    let content = std::fs::read_to_string(path)?;
    let mut added = 0usize;
    let mut updated = 0usize;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let rel: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Skipping malformed relation line: {} ({})", trimmed, e);
                continue;
            }
        };

        let relation_id = rel["relation_id"].as_str().unwrap_or("");
        let rel_type = rel["type"].as_str().unwrap_or("unknown");
        let from_id = rel["from"].as_str().unwrap_or("");
        let to_id = rel["to"].as_str().unwrap_or("");

        if relation_id.is_empty() || from_id.is_empty() || to_id.is_empty() {
            continue;
        }

        let metadata = serde_json::to_string(&rel).unwrap_or_default();
        let now = chrono::Utc::now().to_rfc3339();
        let exists = conn
            .query_row(
                "SELECT id FROM relations WHERE from_entity_id = ?1 AND to_entity_id = ?2 AND relation_type = ?3",
                rusqlite::params![from_id, to_id, rel_type],
                |row| row.get::<_, String>(0),
            )
            .is_ok();

        // Skip relations referencing non-existent entities (FK constraint)
        let from_exists: bool = conn
            .query_row("SELECT 1 FROM entities WHERE id = ?1", rusqlite::params![from_id], |_| {
                Ok(true)
            })
            .unwrap_or(false);
        let to_exists: bool = conn
            .query_row("SELECT 1 FROM entities WHERE id = ?1", rusqlite::params![to_id], |_| {
                Ok(true)
            })
            .unwrap_or(false);
        if !from_exists || !to_exists {
            tracing::warn!("Skipping relation {}: from or to entity not found", relation_id);
            continue;
        }

        let conn_exec = conn.execute(
            "INSERT OR REPLACE INTO relations (id, from_entity_id, to_entity_id, relation_type, metadata, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1.0, ?6)",
            rusqlite::params![relation_id, from_id, to_id, rel_type, metadata, now],
        );
        if let Err(e) = conn_exec {
            tracing::warn!("Failed to insert relation {}: {}", relation_id, e);
            continue;
        }

        if exists {
            updated += 1;
        } else {
            added += 1;
        }
    }

    Ok((added, updated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WorkspaceRegistry;

    #[test]
    fn test_import_ontology_from_temp() {
        let tmp = std::env::temp_dir().join(format!("devbase_onto_{}", std::process::id()));
        let entities_dir = tmp.join("ontology").join("entities");
        let relations_dir = tmp.join("ontology").join("relations");
        std::fs::create_dir_all(&entities_dir).unwrap();
        std::fs::create_dir_all(&relations_dir).unwrap();

        std::fs::write(
            entities_dir.join("person-a.json"),
            r#"{"entity_id":"person-a","type":"person","name":"Alpha","aliases":["a"]}"#,
        )
        .unwrap();
        std::fs::write(
            entities_dir.join("person-b.json"),
            r#"{"entity_id":"person-b","type":"person","name":"Beta","aliases":["b"]}"#,
        )
        .unwrap();
        std::fs::write(
            relations_dir.join("core-relations.jsonl"),
            r#"{"relation_id":"r-test","type":"knows","from":"person-a","to":"person-b"}
{"relation_id":"r-test2","type":"collaborates","from":"person-b","to":"person-a"}"#,
        )
        .unwrap();

        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let stats = import_ontology(&conn, &tmp).unwrap();

        assert_eq!(stats.entities_added, 2);
        assert_eq!(stats.relations_added, 2);
        assert!(stats.errors.is_empty());

        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
