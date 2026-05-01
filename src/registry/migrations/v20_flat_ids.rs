use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v20: Flat ID namespace — remove repo:/skill: prefixes from entities
    // entities becomes the first-class table; repos is maintained by application-layer sync.
    conn.execute("UPDATE entities SET id = SUBSTR(id, 6) WHERE id LIKE 'repo:%'", [])?;
    conn.execute("UPDATE entities SET id = SUBSTR(id, 7) WHERE id LIKE 'skill:%'", [])?;
    // Also flatten any relations that may reference prefixed IDs
    conn.execute(
        "UPDATE relations SET from_entity_id = SUBSTR(from_entity_id, 6) WHERE from_entity_id LIKE 'repo:%'",
        [],
    )?;
    conn.execute(
        "UPDATE relations SET from_entity_id = SUBSTR(from_entity_id, 7) WHERE from_entity_id LIKE 'skill:%'",
        [],
    )?;
    conn.execute(
        "UPDATE relations SET to_entity_id = SUBSTR(to_entity_id, 6) WHERE to_entity_id LIKE 'repo:%'",
        [],
    )?;
    conn.execute(
        "UPDATE relations SET to_entity_id = SUBSTR(to_entity_id, 7) WHERE to_entity_id LIKE 'skill:%'",
        [],
    )?;
    conn.execute("PRAGMA user_version = 20", [])?;
    Ok(())
}
