use rusqlite::{Connection, params};

/// Recompute usage_count, success_rate, and rating for a single skill
/// from its execution history in skill_executions.
pub fn calculate_skill_scores(conn: &Connection, skill_id: &str) -> anyhow::Result<SkillScores> {
    let row = conn.query_row(
        "SELECT COUNT(*),
                SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END),
                AVG(duration_ms)
         FROM skill_executions
         WHERE skill_id = ?1",
        [skill_id],
        |row| {
            let total: i64 = row.get(0)?;
            let success: i64 = row.get(1).unwrap_or(0);
            let avg_duration: Option<f64> = row.get(2)?;
            Ok((total, success, avg_duration))
        },
    )?;

    let (total, success, avg_duration) = row;
    let usage_count = total;
    let success_rate = if total > 0 {
        success as f64 / total as f64
    } else {
        0.0
    };

    // Rating formula:
    //   base = success_rate * 4.0  (0..4)
    //   popularity = min(1.0, ln(usage_count + 1) / 3.0)  (0..1)
    //   speed_bonus = if avg_duration < 1000ms { 0.3 } else if < 5000ms { 0.1 } else { 0.0 }
    //   rating = base + popularity + speed_bonus  (clamped 0..5)
    let popularity = (usage_count as f64 + 1.0).ln() / 3.0;
    let popularity = popularity.min(1.0);

    let speed_bonus = match avg_duration {
        Some(d) if d < 1000.0 => 0.3,
        Some(d) if d < 5000.0 => 0.1,
        _ => 0.0,
    };

    let rating = (success_rate * 4.0 + popularity + speed_bonus).clamp(0.0, 5.0);

    Ok(SkillScores {
        usage_count,
        success_rate,
        rating,
    })
}

/// Update the scores columns in the skills table.
pub fn update_skill_scores(
    conn: &Connection,
    skill_id: &str,
    scores: &SkillScores,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE skills
         SET usage_count = ?1, success_rate = ?2, rating = ?3
         WHERE id = ?4",
        params![scores.usage_count, scores.success_rate, scores.rating, skill_id,],
    )?;
    Ok(())
}

/// Recalculate scores for every skill that has execution records.
pub fn recalculate_all_skill_scores(conn: &Connection) -> anyhow::Result<usize> {
    let mut stmt = conn.prepare("SELECT DISTINCT skill_id FROM skill_executions")?;
    let skill_ids: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut updated = 0;
    for id in &skill_ids {
        let scores = calculate_skill_scores(conn, id)?;
        update_skill_scores(conn, id, &scores)?;
        updated += 1;
    }
    Ok(updated)
}

/// Return the top-N skills ordered by rating (desc), then success_rate (desc).
pub fn get_top_skills(conn: &Connection, limit: usize) -> anyhow::Result<Vec<TopSkill>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, version, description, skill_type, category,
                usage_count, success_rate, rating
         FROM skills
         WHERE rating IS NOT NULL
         ORDER BY rating DESC, success_rate DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], top_skill_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Recommend skills filtered by category, ordered by rating.
pub fn recommend_skills(
    conn: &Connection,
    category: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<TopSkill>> {
    let (sql, params_vec): (&str, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(cat) = category {
        (
            "SELECT id, name, version, description, skill_type, category,
                    usage_count, success_rate, rating
             FROM skills
             WHERE rating IS NOT NULL AND category = ?1
             ORDER BY rating DESC, success_rate DESC
             LIMIT ?2",
            vec![Box::new(cat.to_string()), Box::new(limit as i64)],
        )
    } else {
        (
            "SELECT id, name, version, description, skill_type, category,
                    usage_count, success_rate, rating
             FROM skills
             WHERE rating IS NOT NULL
             ORDER BY rating DESC, success_rate DESC
             LIMIT ?1",
            vec![Box::new(limit as i64)],
        )
    };
    let to_param: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(to_param), top_skill_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn top_skill_from_row(row: &rusqlite::Row) -> rusqlite::Result<TopSkill> {
    Ok(TopSkill {
        id: row.get(0)?,
        name: row.get(1)?,
        version: row.get(2)?,
        description: row.get(3)?,
        skill_type: row.get(4)?,
        category: row.get(5)?,
        usage_count: row.get(6)?,
        success_rate: row.get(7)?,
        rating: row.get(8)?,
    })
}

#[derive(Debug, Clone)]
pub struct SkillScores {
    pub usage_count: i64,
    pub success_rate: f64,
    pub rating: f64,
}

#[derive(Debug, Clone)]
pub struct TopSkill {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub skill_type: String,
    pub category: Option<String>,
    pub usage_count: i64,
    pub success_rate: f64,
    pub rating: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WorkspaceRegistry;
    use crate::skill_runtime::registry::install_skill;
    use crate::skill_runtime::{ExecutionStatus, SkillMeta, SkillType};

    fn dummy_skill_meta(id: &str) -> SkillMeta {
        SkillMeta {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            author: None,
            tags: vec![],
            entry_script: None,
            category: None,
            skill_type: SkillType::Custom,
            local_path: std::path::PathBuf::from(format!("/tmp/{}", id)),
            inputs: vec![],
            outputs: vec![],
            dependencies: vec![],
            embedding: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_used_at: None,
            body: "".to_string(),
        }
    }

    fn record_execution(
        conn: &Connection,
        skill_id: &str,
        status: ExecutionStatus,
        duration_ms: i64,
    ) {
        let started = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO skill_executions (skill_id, args, status, started_at, finished_at, duration_ms)
             VALUES (?1, '', ?2, ?3, ?3, ?4)",
            params![skill_id, status.as_str(), started, duration_ms],
        ).unwrap();
    }

    #[test]
    fn test_calculate_scores() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        install_skill(&conn, &dummy_skill_meta("score-test")).unwrap();

        // 3 successes, 1 failure
        record_execution(&conn, "score-test", ExecutionStatus::Success, 500);
        record_execution(&conn, "score-test", ExecutionStatus::Success, 600);
        record_execution(&conn, "score-test", ExecutionStatus::Success, 700);
        record_execution(&conn, "score-test", ExecutionStatus::Failed, 100);

        let scores = calculate_skill_scores(&conn, "score-test").unwrap();
        assert_eq!(scores.usage_count, 4);
        assert!((scores.success_rate - 0.75).abs() < 0.01);
        assert!(scores.rating > 0.0 && scores.rating <= 5.0);

        update_skill_scores(&conn, "score-test", &scores).unwrap();
    }

    #[test]
    fn test_recalculate_all() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        install_skill(&conn, &dummy_skill_meta("a")).unwrap();
        install_skill(&conn, &dummy_skill_meta("b")).unwrap();
        record_execution(&conn, "a", ExecutionStatus::Success, 200);
        record_execution(&conn, "b", ExecutionStatus::Failed, 200);

        let updated = recalculate_all_skill_scores(&conn).unwrap();
        assert_eq!(updated, 2);

        let top = get_top_skills(&conn, 10).unwrap();
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].id, "a"); // a has higher success_rate
    }

    #[test]
    fn test_recommend_skills() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        install_skill(&conn, &dummy_skill_meta("high")).unwrap();
        install_skill(&conn, &dummy_skill_meta("low")).unwrap();

        // high: 3 successes → high rating
        record_execution(&conn, "high", ExecutionStatus::Success, 200);
        record_execution(&conn, "high", ExecutionStatus::Success, 200);
        record_execution(&conn, "high", ExecutionStatus::Success, 200);

        // low: 1 success, 2 failures → lower rating
        record_execution(&conn, "low", ExecutionStatus::Success, 200);
        record_execution(&conn, "low", ExecutionStatus::Failed, 200);
        record_execution(&conn, "low", ExecutionStatus::Failed, 200);

        let high_scores = calculate_skill_scores(&conn, "high").unwrap();
        update_skill_scores(&conn, "high", &high_scores).unwrap();

        let low_scores = calculate_skill_scores(&conn, "low").unwrap();
        update_skill_scores(&conn, "low", &low_scores).unwrap();

        let recommended = recommend_skills(&conn, None, 10).unwrap();
        assert_eq!(recommended.len(), 2);
        assert_eq!(recommended[0].id, "high");
        assert_eq!(recommended[1].id, "low");
        assert!(recommended[0].rating > recommended[1].rating);
    }
}
