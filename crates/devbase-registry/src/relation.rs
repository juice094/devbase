// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use chrono::Utc;

/// (from_entity_id, to_entity_id, relation_type, confidence, created_at)
pub type RelatedEntityRow = (String, String, String, f64, String);

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
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    } else {
        let mut stmt = conn.prepare(
            "SELECT to_entity_id, relation_type, confidence, created_at FROM relations
             WHERE from_entity_id = ?1
             ORDER BY confidence DESC",
        )?;
        let rows = stmt.query_map([from_entity_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, String>(3)?,
            ))
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
) -> anyhow::Result<Vec<RelatedEntityRow>> {
    let filter_type = relation_type.filter(|s| !s.is_empty());
    if let Some(rt) = filter_type {
        let mut stmt = conn.prepare(
            "SELECT from_entity_id, to_entity_id, relation_type, confidence, created_at FROM relations
             WHERE (from_entity_id = ?1 OR to_entity_id = ?1) AND relation_type = ?2
             ORDER BY confidence DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![entity_id, rt], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    } else {
        let mut stmt = conn.prepare(
            "SELECT from_entity_id, to_entity_id, relation_type, confidence, created_at FROM relations
             WHERE from_entity_id = ?1 OR to_entity_id = ?1
             ORDER BY confidence DESC",
        )?;
        let rows = stmt.query_map([entity_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    fn in_memory() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        conn.execute(
            "CREATE TABLE entities (
                id TEXT PRIMARY KEY
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE relations (
                id TEXT PRIMARY KEY,
                from_entity_id TEXT NOT NULL,
                to_entity_id TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                confidence REAL NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (from_entity_id) REFERENCES entities(id),
                FOREIGN KEY (to_entity_id) REFERENCES entities(id),
                UNIQUE(from_entity_id, to_entity_id, relation_type)
            )",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_save_relation_smoke() {
        let conn = in_memory();
        conn.execute("INSERT INTO entities (id) VALUES ('repo-a'), ('repo-b')", [])
            .unwrap();
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

    #[test]
    fn test_list_relations() {
        let conn = in_memory();
        conn.execute("INSERT INTO entities (id) VALUES ('a'), ('b'), ('c')", [])
            .unwrap();
        super::save_relation(&conn, "a", "b", "depends_on", 0.9).unwrap();
        super::save_relation(&conn, "a", "c", "uses", 0.7).unwrap();

        // List all outgoing relations from 'a'
        let all = super::list_relations(&conn, "a", None).unwrap();
        assert_eq!(all.len(), 2);

        // Filter by relation_type
        let depends = super::list_relations(&conn, "a", Some("depends_on")).unwrap();
        assert_eq!(depends.len(), 1);
        assert_eq!(depends[0].0, "b"); // to_entity_id
        assert_eq!(depends[0].1, "depends_on");
        assert!((depends[0].2 - 0.9).abs() < f64::EPSILON);

        // Empty filter string should behave like None
        let empty_filter = super::list_relations(&conn, "a", Some("")).unwrap();
        assert_eq!(empty_filter.len(), 2);

        // Non-existent from_entity
        let none = super::list_relations(&conn, "z", None).unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn test_find_related_entities_bidirectional() {
        let conn = in_memory();
        conn.execute("INSERT INTO entities (id) VALUES ('a'), ('b'), ('c')", [])
            .unwrap();
        super::save_relation(&conn, "a", "b", "depends_on", 0.9).unwrap();
        super::save_relation(&conn, "c", "a", "uses", 0.8).unwrap();

        // Bidirectional query for 'a' should find both outgoing and incoming
        let related = super::find_related_entities(&conn, "a", None).unwrap();
        assert_eq!(related.len(), 2);

        // Filter by type
        let depends_only = super::find_related_entities(&conn, "a", Some("depends_on")).unwrap();
        assert_eq!(depends_only.len(), 1);
        assert_eq!(depends_only[0].0, "a"); // from_entity_id
        assert_eq!(depends_only[0].1, "b"); // to_entity_id

        // Non-existent entity
        let none = super::find_related_entities(&conn, "z", None).unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn test_save_relation_upsert() {
        let conn = in_memory();
        conn.execute("INSERT INTO entities (id) VALUES ('a'), ('b')", []).unwrap();
        super::save_relation(&conn, "a", "b", "depends_on", 0.5).unwrap();
        super::save_relation(&conn, "a", "b", "depends_on", 0.9).unwrap();

        // Should only have one row with updated confidence
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM relations WHERE from_entity_id = 'a'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);

        let conf: f64 = conn
            .query_row("SELECT confidence FROM relations WHERE from_entity_id = 'a'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!((conf - 0.9).abs() < f64::EPSILON);
    }
}
