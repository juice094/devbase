use super::{
    parse_tags, serialize_tags, ExecutionResult, ExecutionStatus, SkillMeta, SkillRow, SkillType,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

/// Install or update a skill in the registry from a parsed `SkillMeta`.
pub fn install_skill(conn: &Connection, skill: &SkillMeta) -> anyhow::Result<()> {
    let inputs_json = serde_json::to_string(&skill.inputs).unwrap_or_else(|_| "[]".to_string());
    let outputs_json =
        serde_json::to_string(&skill.outputs).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serialize_tags(&skill.tags);
    let embedding_blob = skill
        .embedding
        .as_ref()
        .map(|v| {
            let bytes: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
            bytes
        })
        .unwrap_or_default();

    conn.execute(
        "INSERT INTO skills (
            id, name, version, description, author, tags, entry_script,
            skill_type, local_path, inputs_schema, outputs_schema, embedding,
            installed_at, updated_at, last_used_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
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
            embedding = excluded.embedding,
            updated_at = excluded.updated_at
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
            embedding_blob,
            skill.installed_at.to_rfc3339(),
            skill.updated_at.to_rfc3339(),
            skill.last_used_at.map(|t| t.to_rfc3339()),
        ],
    )?;
    Ok(())
}

/// Remove a skill from the registry (cascades to skill_executions via FK).
pub fn uninstall_skill(conn: &Connection, skill_id: &str) -> anyhow::Result<bool> {
    let rows = conn.execute("DELETE FROM skills WHERE id = ?1", [skill_id])?;
    Ok(rows > 0)
}

/// Retrieve a single skill by ID.
pub fn get_skill(conn: &Connection, skill_id: &str) -> anyhow::Result<Option<SkillRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at
         FROM skills WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map([skill_id], skill_row_from_sql)?;
    rows.next().transpose().map_err(Into::into)
}

/// List all skills, optionally filtered by type.
pub fn list_skills(
    conn: &Connection,
    skill_type: Option<SkillType>,
) -> anyhow::Result<Vec<SkillRow>> {
    let sql = if skill_type.is_some() {
        "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at
         FROM skills WHERE skill_type = ?1 ORDER BY name"
    } else {
        "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at
         FROM skills ORDER BY name"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if let Some(st) = skill_type {
        stmt.query_map([st.as_str()], skill_row_from_sql)?
    } else {
        stmt.query_map([], skill_row_from_sql)?
    };
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Full-text search on skill name and description.
pub fn search_skills_text(conn: &Connection, query: &str, limit: usize) -> anyhow::Result<Vec<SkillRow>> {
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let mut stmt = conn.prepare(
        "SELECT id, name, version, description, author, tags, entry_script,
                skill_type, local_path, installed_at, updated_at, last_used_at
         FROM skills
         WHERE name LIKE ?1 ESCAPE '\\' OR description LIKE ?1 ESCAPE '\\'
         ORDER BY name
         LIMIT ?2"
    )?;
    let rows = stmt.query_map(params![&pattern, limit as i64], skill_row_from_sql)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
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

fn skill_row_from_sql(row: &rusqlite::Row) -> rusqlite::Result<SkillRow> {
    let tags_str: Option<String> = row.get(5)?;
    let skill_type_str: String = row.get(7)?;
    let installed_str: String = row.get(9)?;
    let updated_str: String = row.get(10)?;
    let last_used_str: Option<String> = row.get(11)?;

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
    })
}
