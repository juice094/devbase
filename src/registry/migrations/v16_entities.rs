use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v16: Unified Entity Model — progressive dual-write foundation
    // Entity types define the schema for dynamically-extensible entity kinds.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS entity_types (
            name            TEXT PRIMARY KEY,
            schema_json     TEXT NOT NULL,
            description     TEXT,
            created_at      TEXT NOT NULL
        )",
        [],
    )?;
    // Unified entity storage: repo, skill, paper, vault_note, workflow, etc.
    // v26: added denormalized columns for repo fields (nullable for other entity types).
    conn.execute(
        "CREATE TABLE IF NOT EXISTS entities (
            id              TEXT PRIMARY KEY,
            entity_type     TEXT NOT NULL REFERENCES entity_types(name),
            name            TEXT NOT NULL,
            source_url      TEXT,
            local_path      TEXT,
            metadata        TEXT,
            content_hash    TEXT,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL,
            language        TEXT,
            discovered_at   TEXT,
            workspace_type  TEXT DEFAULT 'git',
            data_tier       TEXT DEFAULT 'private',
            last_synced_at  TEXT,
            stars           INTEGER
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type)",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name)", [])?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_entities_source ON entities(source_url)",
        [],
    )?;
    // Unified relation storage between any two entities.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS relations (
            id              TEXT PRIMARY KEY,
            from_entity_id  TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            to_entity_id    TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            relation_type   TEXT NOT NULL,
            metadata        TEXT,
            confidence      REAL NOT NULL DEFAULT 1.0,
            created_at      TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relations_from ON relations(from_entity_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relations_to ON relations(to_entity_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relations_type ON relations(relation_type)",
        [],
    )?;
    // Seed default entity types for dual-write alignment
    let seed_types = [
        (
            "repo",
            r#"{"fields":[{"name":"language","type":"string"},{"name":"discovered_at","type":"string"},{"name":"workspace_type","type":"string"},{"name":"data_tier","type":"string"},{"name":"stars","type":"integer"}]}"#,
            "Git repository discovered in workspace",
        ),
        (
            "skill",
            r#"{"fields":[{"name":"version","type":"string"},{"name":"author","type":"string"},{"name":"skill_type","type":"string"},{"name":"category","type":"string"},{"name":"entry_script","type":"string"},{"name":"inputs_schema","type":"string"},{"name":"outputs_schema","type":"string"},{"name":"dependencies","type":"string"},{"name":"success_rate","type":"real"},{"name":"usage_count","type":"integer"},{"name":"rating","type":"real"}]}"#,
            "Executable Skill packaged from a project",
        ),
        (
            "paper",
            r#"{"fields":[{"name":"authors","type":"string"},{"name":"venue","type":"string"},{"name":"year","type":"integer"},{"name":"pdf_path","type":"string"},{"name":"bibtex","type":"string"},{"name":"tags","type":"string"}]}"#,
            "Academic paper or publication",
        ),
        (
            "vault_note",
            r#"{"fields":[{"name":"path","type":"string"},{"name":"title","type":"string"},{"name":"frontmatter","type":"string"},{"name":"tags","type":"string"},{"name":"outgoing_links","type":"string"}]}"#,
            "Vault markdown note",
        ),
        (
            "workflow",
            r#"{"fields":[{"name":"definition_json","type":"string"},{"name":"status","type":"string"}]}"#,
            "Multi-Skill orchestration workflow",
        ),
    ];
    let now = chrono::Utc::now().to_rfc3339();
    for (name, schema, desc) in &seed_types {
        conn.execute(
            "INSERT OR IGNORE INTO entity_types (name, schema_json, description, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![name, schema, desc, &now],
        )?;
    }
    // Migrate existing repos → entities (one-way seed)
    let repo_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM repos", [], |row| row.get(0))?;
    let entity_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))?;
    if repo_count > 0 && entity_count == 0 {
        let mut stmt = conn.prepare(
            "SELECT id, local_path, language, discovered_at, workspace_type, data_tier, last_synced_at, stars FROM repos"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<i64>>(7)?,
            ))
        })?;
        for row in rows {
            let (
                id,
                local_path,
                language,
                discovered_at,
                workspace_type,
                data_tier,
                last_synced_at,
                stars,
            ) = row?;
            let metadata = serde_json::json!({
                "language": language,
                "discovered_at": discovered_at,
                "workspace_type": workspace_type,
                "data_tier": data_tier,
                "stars": stars,
                "last_synced_at": last_synced_at,
            });
            conn.execute(
                "INSERT OR IGNORE INTO entities (id, entity_type, name, source_url, local_path, metadata, created_at, updated_at) VALUES (?1, 'repo', ?2, NULL, ?3, ?4, ?5, ?5)",
                rusqlite::params![&id, id.clone(), local_path, metadata.to_string(), &now],
            )?;
        }
    }
    // Migrate existing skills → entities
    let skill_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM skills", [], |row| row.get(0))?;
    if skill_count > 0 && entity_count == 0 {
        let mut stmt = conn.prepare(
            "SELECT id, name, version, author, skill_type, local_path, entry_script, inputs_schema, outputs_schema, dependencies, installed_at, updated_at FROM skills"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
            ))
        })?;
        for row in rows {
            let (
                id,
                name,
                version,
                author,
                skill_type,
                local_path,
                entry_script,
                inputs_schema,
                outputs_schema,
                dependencies,
                installed_at,
                updated_at,
            ) = row?;
            let metadata = serde_json::json!({
                "version": version,
                "author": author,
                "skill_type": skill_type,
                "entry_script": entry_script,
                "inputs_schema": inputs_schema,
                "outputs_schema": outputs_schema,
                "dependencies": dependencies,
            });
            conn.execute(
                "INSERT OR IGNORE INTO entities (id, entity_type, name, source_url, local_path, metadata, created_at, updated_at) VALUES (?1, 'skill', ?2, NULL, ?3, ?4, ?5, ?6)",
                rusqlite::params![&id, name, local_path, metadata.to_string(), installed_at, updated_at],
            )?;
        }
    }
    // Extend skills table with category + rating reservation
    let skill_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(skills)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        rows.filter_map(Result::ok).collect()
    };
    if !skill_cols.iter().any(|c| c == "category") {
        conn.execute("ALTER TABLE skills ADD COLUMN category TEXT", [])?;
    }
    if !skill_cols.iter().any(|c| c == "success_rate") {
        conn.execute("ALTER TABLE skills ADD COLUMN success_rate REAL", [])?;
    }
    if !skill_cols.iter().any(|c| c == "usage_count") {
        conn.execute("ALTER TABLE skills ADD COLUMN usage_count INTEGER DEFAULT 0", [])?;
    }
    if !skill_cols.iter().any(|c| c == "rating") {
        conn.execute("ALTER TABLE skills ADD COLUMN rating REAL", [])?;
    }
    conn.execute("PRAGMA user_version = 16", [])?;
    Ok(())
}
