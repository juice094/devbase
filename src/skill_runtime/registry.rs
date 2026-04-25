use super::{
    parse_tags, serialize_tags, ExecutionResult, ExecutionStatus, SkillMeta, SkillRow, SkillType,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

/// Clone a skill from a Git URL and install it into the devbase skills directory.
pub fn install_skill_from_git(
    conn: &Connection,
    git_url: &str,
    skill_id: Option<&str>,
) -> anyhow::Result<SkillMeta> {
    let skills_dir = crate::registry::WorkspaceRegistry::workspace_dir()?
        .join("skills");
    std::fs::create_dir_all(&skills_dir)?;

    // Derive skill ID from URL or provided name
    let id = skill_id.map(|s| s.to_string()).unwrap_or_else(|| {
        git_url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("skill")
            .trim_end_matches(".git")
            .to_lowercase()
            .replace('_', "-")
    });

    let target_dir = skills_dir.join(&id);

    // Remove existing directory if present
    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir)?;
    }

    // Clone repository
    git2::Repository::clone(git_url, &target_dir)
        .map_err(|e| anyhow::anyhow!("Git clone failed: {}", e))?;

    // Parse SKILL.md
    let skill_md = target_dir.join("SKILL.md");
    if !skill_md.exists() {
        return Err(anyhow::anyhow!(
            "Cloned repository does not contain SKILL.md at: {}",
            skill_md.display()
        ));
    }

    let mut skill = crate::skill_runtime::parser::parse_skill_md(&skill_md)?;
    skill.id = id;
    skill.local_path = target_dir;
    skill.skill_type = SkillType::Custom;

    install_skill(conn, &skill)?;
    Ok(skill)
}

/// Install or update a skill in the registry from a parsed `SkillMeta`.
pub fn install_skill(conn: &Connection, skill: &SkillMeta) -> anyhow::Result<()> {
    let inputs_json = serde_json::to_string(&skill.inputs).unwrap_or_else(|_| "[]".to_string());
    let outputs_json =
        serde_json::to_string(&skill.outputs).unwrap_or_else(|_| "[]".to_string());
    let deps_json = serde_json::to_string(&skill.dependencies).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serialize_tags(&skill.tags);
    let embedding_blob = skill
        .embedding
        .as_ref()
        .map(|v| {
            let bytes: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
            bytes
        })
        .unwrap_or_default();

    // Atomic dual-write: both skills and entities in a single transaction
    let tx = conn.unchecked_transaction()?;

    tx.execute(
        "INSERT INTO skills (
            id, name, version, description, author, tags, entry_script,
            skill_type, local_path, inputs_schema, outputs_schema, dependencies, embedding,
            installed_at, updated_at, last_used_at, category, success_rate, usage_count, rating
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            version = excluded.version,
            description = excluded.description,
            author = excluded.author,
            tags = excluded.tags,
            entry_script = excluded.entry_script,
            skill_type = excluded.skill_type,
            local_path = excluded.local_path,
            inputs_schema = excluded.inputs_schema,
            outputs_schema = excluded.outputs_schema,
            dependencies = excluded.dependencies,
            embedding = excluded.embedding,
            updated_at = excluded.updated_at,
            category = excluded.category,
            success_rate = excluded.success_rate,
            usage_count = excluded.usage_count,
            rating = excluded.rating
        ",
        params![
            &skill.id,
            &skill.name,
            &skill.version,
            &skill.description,
            skill.author.as_deref(),
            &tags_json,
            skill.entry_script.as_deref(),
            skill.skill_type.as_str(),
            skill.local_path.to_string_lossy().to_string(),
            &inputs_json,
            &outputs_json,
            &deps_json,
            embedding_blob,
            skill.installed_at.to_rfc3339(),
            skill.updated_at.to_rfc3339(),
            skill.last_used_at.map(|t| t.to_rfc3339()),
            skill.category.as_deref(),
            None::<f64>,    // success_rate: reserved for v0.6.0
            0i64,           // usage_count: reserved for v0.6.0
            None::<f64>,    // rating: reserved for v0.6.0
        ],
    )?;

    // Dual-write: sync to unified entities table
    sync_skill_to_entities(&tx, skill)?;

    tx.commit()?;
    Ok(())
}

/// Dual-write helper: mirror a Skill into the unified entities table.
fn sync_skill_to_entities(conn: &Connection, skill: &SkillMeta) -> anyhow::Result<()> {
    let metadata = serde_json::json!({
        "version": skill.version,
        "author": skill.author,
        "skill_type": skill.skill_type.as_str(),
        "category": skill.category,
        "entry_script": skill.entry_script,
        "inputs_schema": serde_json::to_string(&skill.inputs).unwrap_or_else(|_| "[]".to_string()),
        "outputs_schema": serde_json::to_string(&skill.outputs).unwrap_or_else(|_| "[]".to_string()),
        "dependencies": serde_json::to_string(&skill.dependencies).unwrap_or_else(|_| "[]".to_string()),
    });
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, source_url, local_path, metadata, created_at, updated_at)
         VALUES (?1, 'skill', ?2, NULL, ?3, ?4, ?5, ?5)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            local_path = excluded.local_path,
            metadata = excluded.metadata,
            updated_at = excluded.updated_at",
        params![
            format!("skill:{}", skill.id),
            &skill.name,
            skill.local_path.to_string_lossy().to_string(),
            metadata.to_string(),
            &now,
        ],
    )?;
    Ok(())
}

/// Remove a skill from the registry (cascades to skill_executions via FK).
pub fn uninstall_skill(conn: &Connection, skill_id: &str) -> anyhow::Result<bool> {
    let rows = conn.execute("DELETE FROM skills WHERE id = ?1", [skill_id])?;
    // Dual-write: also remove from entities table
    let _ = conn.execute("DELETE FROM entities WHERE id = ?1", [format!("skill:{}", skill_id)]);
    Ok(rows > 0)
}

/// Retrieve a single skill by ID.
pub fn get_skill(conn: &Connection, skill_id: &str) -> anyhow::Result<Option<SkillRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at, dependencies, category
         FROM skills WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map([skill_id], skill_row_from_sql)?;
    rows.next().transpose().map_err(Into::into)
}

/// List all skills, optionally filtered by type.
pub fn list_skills(
    conn: &Connection,
    skill_type: Option<SkillType>,
    category: Option<&str>,
) -> anyhow::Result<Vec<SkillRow>> {
    let sql = match (skill_type, category) {
        (Some(_), Some(_)) => "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at, dependencies, category
         FROM skills WHERE skill_type = ?1 AND category = ?2 ORDER BY name",
        (Some(_), None) => "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at, dependencies, category
         FROM skills WHERE skill_type = ?1 ORDER BY name",
        (None, Some(_)) => "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at, dependencies, category
         FROM skills WHERE category = ?1 ORDER BY name",
        (None, None) => "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at, dependencies, category
         FROM skills ORDER BY name",
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = match (skill_type, category) {
        (Some(st), Some(cat)) => stmt.query_map(params![st.as_str(), cat], skill_row_from_sql)?,
        (Some(st), None) => stmt.query_map([st.as_str()], skill_row_from_sql)?,
        (None, Some(cat)) => stmt.query_map([cat], skill_row_from_sql)?,
        (None, None) => stmt.query_map([], skill_row_from_sql)?,
    };
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Full-text search on skill name and description.
pub fn search_skills_text(
    conn: &Connection,
    query: &str,
    limit: usize,
    category: Option<&str>,
) -> anyhow::Result<Vec<SkillRow>> {
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let sql = if category.is_some() {
        "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at, dependencies, category
         FROM skills
         WHERE (name LIKE ?1 ESCAPE '\\' OR description LIKE ?1 ESCAPE '\\') AND category = ?2
         ORDER BY name
         LIMIT ?3"
    } else {
        "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at, dependencies, category
         FROM skills
         WHERE name LIKE ?1 ESCAPE '\\' OR description LIKE ?1 ESCAPE '\\'
         ORDER BY name
         LIMIT ?2"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if let Some(cat) = category {
        stmt.query_map(params![&pattern, cat, limit as i64], skill_row_from_sql)?
    } else {
        stmt.query_map(params![&pattern, limit as i64], skill_row_from_sql)?
    };
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Semantic search on skill descriptions using cosine similarity.
pub fn search_skills_semantic(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
    category: Option<&str>,
) -> anyhow::Result<Vec<SkillRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at, dependencies, category, embedding
         FROM skills
         WHERE embedding IS NOT NULL AND LENGTH(embedding) > 0"
    )?;

    let mut scored: Vec<(f32, SkillRow)> = Vec::new();

    let rows = stmt.query_map([], |row| {
        let skill = skill_row_from_sql(row)?;
        let blob: Vec<u8> = row.get(14)?;
        let emb: Vec<f32> = blob
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let score = cosine_similarity(query_embedding, &emb);
        Ok((score, skill))
    })?;

    for row in rows {
        let (score, skill) = row?;
        if let Some(cat) = category {
            if skill.category.as_deref() != Some(cat) {
                continue;
            }
        }
        scored.push((score, skill));
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(scored.into_iter().take(limit).map(|(_, s)| s).collect())
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Record the start of a skill execution.
pub fn record_execution_start(
    conn: &Connection,
    skill_id: &str,
    args: &str,
) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO skill_executions (skill_id, args, status, started_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![skill_id, args, ExecutionStatus::Running.as_str(), Utc::now().to_rfc3339()],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Update an execution record with results.
pub fn record_execution_finish(
    conn: &Connection,
    execution_id: i64,
    result: &ExecutionResult,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE skill_executions
         SET status = ?1, stdout = ?2, stderr = ?3, exit_code = ?4,
             finished_at = ?5, duration_ms = ?6
         WHERE id = ?7",
        params![
            result.status.as_str(),
            &result.stdout,
            &result.stderr,
            result.exit_code,
            Utc::now().to_rfc3339(),
            result.duration_ms as i64,
            execution_id,
        ],
    )?;

    // Update last_used_at on the skill
    conn.execute(
        "UPDATE skills SET last_used_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), &result.skill_id],
    )?;
    Ok(())
}

/// List recent executions for a skill.
pub fn list_executions(
    conn: &Connection,
    skill_id: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<ExecutionRecord>> {
    let (sql, params_vec) = if let Some(id) = skill_id {
        (
            "SELECT id, skill_id, args, status, stdout, stderr, exit_code,
                    started_at, finished_at, duration_ms
             FROM skill_executions WHERE skill_id = ?1 ORDER BY started_at DESC LIMIT ?2",
            vec![id.to_string(), limit.to_string()],
        )
    } else {
        (
            "SELECT id, skill_id, args, status, stdout, stderr, exit_code,
                    started_at, finished_at, duration_ms
             FROM skill_executions ORDER BY started_at DESC LIMIT ?1",
            vec![limit.to_string()],
        )
    };
    let mut stmt = conn.prepare(sql)?;
    let to_param: Vec<&dyn rusqlite::ToSql> =
        params_vec.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(&*to_param, |row| {
        Ok(ExecutionRecord {
            id: row.get(0)?,
            skill_id: row.get(1)?,
            args: row.get(2)?,
            status: row.get::<_, String>(3)?.parse().unwrap_or(ExecutionStatus::Failed),
            stdout: row.get(4)?,
            stderr: row.get(5)?,
            exit_code: row.get(6)?,
            started_at: row.get::<_, String>(7)?.parse().ok(),
            finished_at: row.get::<_, String>(8)?.parse().ok(),
            duration_ms: row.get(9)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Row type for execution history queries.
#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub id: i64,
    pub skill_id: String,
    pub args: Option<String>,
    pub status: ExecutionStatus,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WorkspaceRegistry;

    fn dummy_skill(id: &str, name: &str, skill_type: SkillType) -> SkillMeta {
        SkillMeta {
            id: id.to_string(),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: format!("A {} skill", name),
            author: Some("test".to_string()),
            tags: vec!["test".to_string()],
            entry_script: Some("scripts/run.py".to_string()),
            category: None,
            skill_type,
            local_path: std::path::PathBuf::from(format!("/tmp/skills/{}", id)),
            inputs: vec![],
            outputs: vec![],
            dependencies: vec![],
            embedding: None,
            installed_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
            body: "# Test".to_string(),
        }
    }

    #[test]
    fn test_install_and_get_skill() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let skill = dummy_skill("test-skill", "Test Skill", SkillType::Custom);
        install_skill(&conn, &skill).unwrap();

        let row = get_skill(&conn, "test-skill").unwrap().unwrap();
        assert_eq!(row.id, "test-skill");
        assert_eq!(row.name, "Test Skill");
        assert_eq!(row.skill_type, SkillType::Custom);
        assert_eq!(row.tags, vec!["test"]);
    }

    #[test]
    fn test_list_skills_by_type() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        install_skill(&conn, &dummy_skill("builtin-a", "Builtin A", SkillType::Builtin)).unwrap();
        install_skill(&conn, &dummy_skill("custom-a", "Custom A", SkillType::Custom)).unwrap();
        install_skill(&conn, &dummy_skill("builtin-b", "Builtin B", SkillType::Builtin)).unwrap();

        let all = list_skills(&conn, None, None).unwrap();
        assert_eq!(all.len(), 3);

        let builtins = list_skills(&conn, Some(SkillType::Builtin), None).unwrap();
        assert_eq!(builtins.len(), 2);

        let customs = list_skills(&conn, Some(SkillType::Custom), None).unwrap();
        assert_eq!(customs.len(), 1);
    }

    #[test]
    fn test_uninstall_skill() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        install_skill(&conn, &dummy_skill("to-remove", "Remove Me", SkillType::Custom)).unwrap();
        assert!(get_skill(&conn, "to-remove").unwrap().is_some());

        let removed = uninstall_skill(&conn, "to-remove").unwrap();
        assert!(removed);
        assert!(get_skill(&conn, "to-remove").unwrap().is_none());

        let not_found = uninstall_skill(&conn, "nonexistent").unwrap();
        assert!(!not_found);
    }

    #[test]
    fn test_search_skills_text() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        install_skill(&conn, &dummy_skill("code-audit", "Code Audit", SkillType::Custom)).unwrap();
        install_skill(&conn, &dummy_skill("embed-repo", "Embed Repo", SkillType::Builtin)).unwrap();

        let results = search_skills_text(&conn, "audit", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "code-audit");

        let results = search_skills_text(&conn, "repo", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "embed-repo");
    }

    #[test]
    fn test_execution_tracking() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let skill = dummy_skill("tracked", "Tracked", SkillType::Custom);
        install_skill(&conn, &skill).unwrap();

        let exec_id = record_execution_start(&conn, "tracked", "{\"x\":1}").unwrap();
        assert!(exec_id > 0);

        let result = ExecutionResult {
            skill_id: "tracked".to_string(),
            status: ExecutionStatus::Success,
            stdout: "hello".to_string(),
            stderr: "".to_string(),
            exit_code: Some(0),
            duration_ms: 100,
        };
        record_execution_finish(&conn, exec_id, &result).unwrap();

        let history = list_executions(&conn, Some("tracked"), 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, ExecutionStatus::Success);
        assert_eq!(history[0].stdout, Some("hello".to_string()));

        // last_used_at should be updated
        let row = get_skill(&conn, "tracked").unwrap().unwrap();
        assert!(row.last_used_at.is_some());
    }
}

fn skill_row_from_sql(row: &rusqlite::Row) -> rusqlite::Result<SkillRow> {
    let tags_str: Option<String> = row.get(5)?;
    let skill_type_str: String = row.get(7)?;
    let installed_str: String = row.get(9)?;
    let updated_str: String = row.get(10)?;
    let last_used_str: Option<String> = row.get(11)?;
    let deps_str: Option<String> = row.get(12)?;
    let category: Option<String> = row.get(13)?;

    Ok(SkillRow {
        id: row.get(0)?,
        name: row.get(1)?,
        version: row.get(2)?,
        description: row.get(3)?,
        author: row.get(4)?,
        tags: parse_tags(tags_str.as_deref()),
        entry_script: row.get(6)?,
        skill_type: skill_type_str
            .parse()
            .unwrap_or(SkillType::Custom),
        local_path: row.get(8)?,
        installed_at: installed_str.parse().unwrap_or_else(|_| Utc::now()),
        updated_at: updated_str.parse().unwrap_or_else(|_| Utc::now()),
        last_used_at: last_used_str.and_then(|s| s.parse().ok()),
        dependencies: parse_dependencies(deps_str.as_deref()),
        category,
    })
}

fn parse_dependencies(deps_str: Option<&str>) -> Vec<crate::skill_runtime::SkillDependency> {
    let Some(s) = deps_str else {
        return Vec::new();
    };
    serde_json::from_str(s).unwrap_or_default()
}
