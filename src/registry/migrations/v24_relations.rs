use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v24: Activate the unified relations table.
    // 1. Enforce uniqueness so ON CONFLICT works for idempotent writes.
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_relations_unique ON relations(from_entity_id, to_entity_id, relation_type)",
        [],
    )?;
    // 2. One-time migration: copy existing repo_relations into relations.
    //    Only rows whose endpoints exist in entities are migrated.
    conn.execute(
        "INSERT INTO relations (id, from_entity_id, to_entity_id, relation_type, confidence, created_at)
         SELECT lower(hex(randomblob(16))), from_repo_id, to_repo_id, relation_type, confidence, discovered_at
         FROM repo_relations
         WHERE from_repo_id IN (SELECT id FROM entities) AND to_repo_id IN (SELECT id FROM entities)
         ON CONFLICT(from_entity_id, to_entity_id, relation_type) DO UPDATE SET
             confidence = excluded.confidence,
             created_at = excluded.created_at",
        [],
    )?;
    conn.execute("PRAGMA user_version = 24", [])?;
    Ok(())
}
