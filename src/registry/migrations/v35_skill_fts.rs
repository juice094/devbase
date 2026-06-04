use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS skills_fts USING fts5(
            name,
            description,
            tags,
            category,
            content='skills',
            content_rowid='rowid',
            tokenize='unicode61'
        );

        CREATE TRIGGER IF NOT EXISTS skills_fts_ai AFTER INSERT ON skills BEGIN
            INSERT INTO skills_fts(rowid, name, description, tags, category)
            VALUES (new.rowid, new.name, new.description, new.tags, new.category);
        END;

        CREATE TRIGGER IF NOT EXISTS skills_fts_ad AFTER DELETE ON skills BEGIN
            INSERT INTO skills_fts(skills_fts, rowid, name, description, tags, category)
            VALUES ('delete', old.rowid, old.name, old.description, old.tags, old.category);
        END;

        CREATE TRIGGER IF NOT EXISTS skills_fts_au AFTER UPDATE ON skills BEGIN
            INSERT INTO skills_fts(skills_fts, rowid, name, description, tags, category)
            VALUES ('delete', old.rowid, old.name, old.description, old.tags, old.category);
            INSERT INTO skills_fts(rowid, name, description, tags, category)
            VALUES (new.rowid, new.name, new.description, new.tags, new.category);
        END;",
    )?;

    conn.execute("PRAGMA user_version = 35", [])?;
    Ok(())
}
