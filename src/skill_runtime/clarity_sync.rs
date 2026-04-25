//! Sync devbase skills to Clarity plans.

use std::path::Path;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Clarity plan step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarityPlanStep {
    pub id: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

/// Clarity plan JSON format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarityPlan {
    pub id: String,
    pub title: String,
    pub description: String,
    pub steps: Vec<ClarityPlanStep>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// A devbase skill row with inputs schema.
struct SkillWithInputs {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: String, // JSON or CSV string
    pub inputs_schema: String,
    pub updated_at: DateTime<Utc>,
}

/// Sync all skills from devbase to Clarity plans directory.
pub fn sync_skills_to_clarity(conn: &Connection, clarity_dir: &Path) -> Result<usize> {
    let plans_dir = clarity_dir.join("plans");
    std::fs::create_dir_all(&plans_dir)
        .with_context(|| format!("Failed to create Clarity plans dir: {}", plans_dir.display()))?;

    let skills = fetch_skills_with_inputs(conn)?;
    let mut synced = 0;

    for skill in skills {
        let plan_path = plans_dir.join(format!("{}.json", skill.id));

        let devbase_updated = skill.updated_at;

        // Conflict resolution: if plan exists, compare updated_at
        if plan_path.exists() {
            let existing_content = std::fs::read_to_string(&plan_path)
                .with_context(|| format!("Failed to read existing plan: {}", plan_path.display()))?;
            let existing_plan: ClarityPlan = serde_json::from_str(&existing_content)
                .with_context(|| format!("Failed to parse existing plan: {}", plan_path.display()))?;

            if let Some(existing_updated_str) = &existing_plan.updated_at {
                if let Ok(existing_updated) = DateTime::parse_from_rfc3339(existing_updated_str) {
                    let existing_updated = existing_updated.with_timezone(&Utc);
                    if existing_updated >= devbase_updated {
                        // Existing plan is newer or same, skip
                        continue;
                    }
                }
            } else if let Ok(existing_created) = DateTime::parse_from_rfc3339(&existing_plan.created_at) {
                let existing_created = existing_created.with_timezone(&Utc);
                if existing_created >= devbase_updated {
                    continue;
                }
            }
        }

        let plan = skill_to_plan(&skill);
        let json = serde_json::to_string_pretty(&plan)
            .with_context(|| format!("Failed to serialize plan for skill {}", skill.id))?;
        std::fs::write(&plan_path, json)
            .with_context(|| format!("Failed to write plan: {}", plan_path.display()))?;
        synced += 1;
    }

    Ok(synced)
}

fn fetch_skills_with_inputs(conn: &Connection) -> Result<Vec<SkillWithInputs>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, tags, inputs_schema, updated_at FROM skills ORDER BY name"
    )?;
    let rows = stmt.query_map([], |row| {
        let updated_str: String = row.get(5)?;
        let updated_at = updated_str.parse().unwrap_or_else(|_| Utc::now());
        Ok(SkillWithInputs {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            tags: row.get(3)?,
            inputs_schema: row.get(4)?,
            updated_at,
        })
    })?;
    let mut skills = Vec::new();
    for row in rows {
        skills.push(row?);
    }
    Ok(skills)
}

fn skill_to_plan(skill: &SkillWithInputs) -> ClarityPlan {
    let inputs: Vec<serde_json::Value> = serde_json::from_str(&skill.inputs_schema).unwrap_or_default();

    let steps: Vec<ClarityPlanStep> = inputs
        .iter()
        .enumerate()
        .map(|(i, input)| {
            let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let desc = input.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let required = input.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
            let input_type = input.get("input_type").and_then(|v| v.as_str()).unwrap_or("string");

            let step_desc = if desc.is_empty() {
                format!("Parameter: {} ({})", name, input_type)
            } else {
                format!("{}: {} ({})", name, desc, input_type)
            };
            
            let step_desc = if required {
                format!("{} [required]", step_desc)
            } else {
                step_desc
            };

            ClarityPlanStep {
                id: format!("step_{}", i + 1),
                description: step_desc,
                status: "pending".to_string(),
                result: None,
            }
        })
        .collect();

    let now = Utc::now().to_rfc3339();
    
    let mut description = skill.description.clone();
    let tags = parse_tags_from_skill(&skill.tags);
    if !tags.is_empty() {
        if !description.is_empty() {
            description.push('\n');
            description.push('\n');
        }
        description.push_str(&format!("Tags: {}", tags.join(", ")));
    }

    ClarityPlan {
        id: skill.id.clone(),
        title: skill.name.clone(),
        description,
        steps,
        created_at: now.clone(),
        updated_at: Some(now),
    }
}

fn parse_tags_from_skill(tags_str: &str) -> Vec<String> {
    if tags_str.trim().starts_with('[') {
        serde_json::from_str(tags_str).unwrap_or_default()
    } else {
        tags_str.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WorkspaceRegistry;
    use crate::skill_runtime::{SkillMeta, SkillType, SkillInput};
    use chrono::Utc;

    #[test]
    fn test_sync_skills_to_clarity() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let skill = SkillMeta {
            id: "test-skill".to_string(),
            name: "Test Skill".to_string(),
            version: "1.0.0".to_string(),
            description: "A test skill".to_string(),
            author: Some("test".to_string()),
            tags: vec!["rust".to_string(), "deploy".to_string()],
            entry_script: None,
            category: None,
            skill_type: SkillType::Custom,
            local_path: std::path::PathBuf::from("/tmp/skills/test-skill"),
            inputs: vec![
                SkillInput {
                    name: "path".to_string(),
                    input_type: "string".to_string(),
                    description: "Target path".to_string(),
                    required: true,
                    default: None,
                }
            ],
            outputs: vec![],
            dependencies: vec![],
            embedding: None,
            installed_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
            body: "".to_string(),
        };
        crate::skill_runtime::registry::install_skill(&conn, &skill).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let clarity_dir = tmp.path();
        std::fs::create_dir_all(clarity_dir.join("plans")).unwrap();

        let count = sync_skills_to_clarity(&conn, clarity_dir).unwrap();
        assert_eq!(count, 1);

        let plan_path = clarity_dir.join("plans").join("test-skill.json");
        assert!(plan_path.exists());
        let content = std::fs::read_to_string(&plan_path).unwrap();
        let plan: ClarityPlan = serde_json::from_str(&content).unwrap();
        assert_eq!(plan.id, "test-skill");
        assert_eq!(plan.title, "Test Skill");
        assert!(plan.description.contains("A test skill"));
        assert!(plan.description.contains("rust"));
        assert!(plan.description.contains("deploy"));
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].id, "step_1");
        assert!(plan.steps[0].description.contains("path"));
    }

    #[test]
    fn test_conflict_resolution_skips_older_devbase_skill() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let skill = SkillMeta {
            id: "conflict-test".to_string(),
            name: "Conflict Test".to_string(),
            version: "1.0.0".to_string(),
            description: "New desc".to_string(),
            author: None,
            tags: vec![],
            entry_script: None,
            category: None,
            skill_type: SkillType::Custom,
            local_path: std::path::PathBuf::from("/tmp/skills/conflict-test"),
            inputs: vec![],
            outputs: vec![],
            dependencies: vec![],
            embedding: None,
            installed_at: Utc::now() - chrono::Duration::days(2),
            updated_at: Utc::now() - chrono::Duration::days(2),
            last_used_at: None,
            body: "".to_string(),
        };
        crate::skill_runtime::registry::install_skill(&conn, &skill).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let clarity_dir = tmp.path();
        let plans_dir = clarity_dir.join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();

        // Write an existing plan with a newer updated_at
        let existing_plan = ClarityPlan {
            id: "conflict-test".to_string(),
            title: "Conflict Test".to_string(),
            description: "Old desc".to_string(),
            steps: vec![],
            created_at: (Utc::now() - chrono::Duration::days(1)).to_rfc3339(),
            updated_at: Some(Utc::now().to_rfc3339()),
        };
        std::fs::write(
            plans_dir.join("conflict-test.json"),
            serde_json::to_string_pretty(&existing_plan).unwrap(),
        ).unwrap();

        let count = sync_skills_to_clarity(&conn, clarity_dir).unwrap();
        assert_eq!(count, 0);

        let content = std::fs::read_to_string(plans_dir.join("conflict-test.json")).unwrap();
        let plan: ClarityPlan = serde_json::from_str(&content).unwrap();
        assert_eq!(plan.description, "Old desc");
    }

    #[test]
    fn test_conflict_resolution_updates_when_devbase_newer() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let skill = SkillMeta {
            id: "update-test".to_string(),
            name: "Update Test".to_string(),
            version: "1.0.0".to_string(),
            description: "New desc".to_string(),
            author: None,
            tags: vec![],
            entry_script: None,
            category: None,
            skill_type: SkillType::Custom,
            local_path: std::path::PathBuf::from("/tmp/skills/update-test"),
            inputs: vec![],
            outputs: vec![],
            dependencies: vec![],
            embedding: None,
            installed_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
            body: "".to_string(),
        };
        crate::skill_runtime::registry::install_skill(&conn, &skill).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let clarity_dir = tmp.path();
        let plans_dir = clarity_dir.join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();

        // Write an existing plan with an older updated_at
        let existing_plan = ClarityPlan {
            id: "update-test".to_string(),
            title: "Update Test".to_string(),
            description: "Old desc".to_string(),
            steps: vec![],
            created_at: (Utc::now() - chrono::Duration::days(2)).to_rfc3339(),
            updated_at: Some((Utc::now() - chrono::Duration::days(1)).to_rfc3339()),
        };
        std::fs::write(
            plans_dir.join("update-test.json"),
            serde_json::to_string_pretty(&existing_plan).unwrap(),
        ).unwrap();

        let count = sync_skills_to_clarity(&conn, clarity_dir).unwrap();
        assert_eq!(count, 1);

        let content = std::fs::read_to_string(plans_dir.join("update-test.json")).unwrap();
        let plan: ClarityPlan = serde_json::from_str(&content).unwrap();
        assert_eq!(plan.description, "New desc");
    }
}
