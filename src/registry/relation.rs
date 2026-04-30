use chrono::Utc;

/// Store a directed relation between two entities.
/// Upserts on conflict (from, to, type) to update confidence and timestamp.
pub fn save_relation(
    conn: &rusqlite::Connection,
    from: &str,
    to: &str,
    rel_type: &str,
    confidence: f64,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO relations (id, from_entity_id, to_entity_id, relation_type, confidence, created_at)
         VALUES (lower(hex(randomblob(16))), ?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(from_entity_id, to_entity_id, relation_type) DO UPDATE SET
             confidence = excluded.confidence,
             created_at = excluded.created_at",
        rusqlite::params![from, to, rel_type, confidence, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

/// Query outgoing relations from a given entity.
/// Optionally filter by relation_type (pass None or empty for all types).
pub fn list_relations(
    conn: &rusqlite::Connection,
    from_entity_id: &str,
    relation_type: Option<&str>,
) -> anyhow::Result<Vec<(String, String, f64, String)>> {
    let filter_type = relation_type.filter(|s| !s.is_empty());
    if let Some(rt) = filter_type {
        let mut stmt = conn.prepare(
            "SELECT to_entity_id, relation_type, confidence, created_at FROM relations
             WHERE from_entity_id = ?1 AND relation_type = ?2
             ORDER BY confidence DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![from_entity_id, rt], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?, row.get::<_, String>(3)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    } else {
        let mut stmt = conn.prepare(
            "SELECT to_entity_id, relation_type, confidence, created_at FROM relations
             WHERE from_entity_id = ?1
             ORDER BY confidence DESC",
        )?;
        let rows = stmt.query_map([from_entity_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?, row.get::<_, String>(3)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

/// Query bidirectional relations for an entity (both outgoing and incoming).
/// Optionally filter by relation_type.
pub fn find_related_entities(
    conn: &rusqlite::Connection,
    entity_id: &str,
    relation_type: Option<&str>,
) -> anyhow::Result<Vec<(String, String, String, f64, String)>> {
    let filter_type = relation_type.filter(|s| !s.is_empty());
    if let Some(rt) = filter_type {
        let mut stmt = conn.prepare(
            "SELECT from_entity_id, to_entity_id, relation_type, confidence, created_at FROM relations
             WHERE (from_entity_id = ?1 OR to_entity_id = ?1) AND relation_type = ?2
             ORDER BY confidence DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![entity_id, rt], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, f64>(3)?, row.get::<_, String>(4)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    } else {
        let mut stmt = conn.prepare(
            "SELECT from_entity_id, to_entity_id, relation_type, confidence, created_at FROM relations
             WHERE from_entity_id = ?1 OR to_entity_id = ?1
             ORDER BY confidence DESC",
        )?;
        let rows = stmt.query_map([entity_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, f64>(3)?, row.get::<_, String>(4)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use crate::registry::WorkspaceRegistry;

    #[test]
    fn test_save_relation_smoke() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-b").unwrap();
        super::save_relation(&conn, "repo-a", "repo-b", "depends_on", 0.95).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM relations WHERE from_entity_id = ?1 AND to_entity_id = ?2",
                ["repo-a", "repo-b"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
