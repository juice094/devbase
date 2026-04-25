use super::model::{ExecutionStatus, WorkflowDefinition, WorkflowExecution};
use rusqlite::{params, Connection};

pub fn save_workflow(conn: &Connection, wf: &WorkflowDefinition) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let yaml = serde_yaml::to_string(wf)?;
    conn.execute(
        "INSERT INTO workflows (id, name, version, description, definition_yaml, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6, ?6)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            version = excluded.version,
            description = excluded.description,
            definition_yaml = excluded.definition_yaml,
            updated_at = excluded.updated_at",
        params![&wf.id, &wf.name, &wf.version, wf.description.as_deref().unwrap_or(""), yaml, now],
    )?;
    Ok(())
}

pub fn get_workflow(conn: &Connection, id: &str) -> anyhow::Result<Option<WorkflowDefinition>> {
    let mut stmt = conn.prepare(
        "SELECT definition_yaml FROM workflows WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map([id], |row| {
        let yaml: String = row.get(0)?;
        Ok(yaml)
    })?;
    if let Some(yaml) = rows.next().transpose()? {
        let wf: WorkflowDefinition = serde_yaml::from_str(&yaml)?;
        Ok(Some(wf))
    } else {
        Ok(None)
    }
}

pub fn list_workflows(conn: &Connection) -> anyhow::Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, version FROM workflows ORDER BY name"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn delete_workflow(conn: &Connection, id: &str) -> anyhow::Result<bool> {
    let rows = conn.execute("DELETE FROM workflows WHERE id = ?1", [id])?;
    Ok(rows > 0)
}

pub fn create_execution(
    conn: &Connection,
    workflow_id: &str,
    inputs_json: &str,
) -> anyhow::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO workflow_executions (workflow_id, inputs_json, status, current_step, started_at)
         VALUES (?1, ?2, 'Pending', NULL, ?3)",
        params![workflow_id, inputs_json, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_execution(
    conn: &Connection,
    exec_id: i64,
    status: &ExecutionStatus,
    current_step: Option<&str>,
    duration_ms: Option<i64>,
) -> anyhow::Result<()> {
    let status_str = format!("{:?}", status);
    let finished_at = if matches!(status, ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::Cancelled) {
        Some(chrono::Utc::now().to_rfc3339())
    } else {
        None
    };
    conn.execute(
        "UPDATE workflow_executions
         SET status = ?1, current_step = ?2, duration_ms = ?3, finished_at = ?4
         WHERE id = ?5",
        params![status_str, current_step, duration_ms, finished_at, exec_id],
    )?;
    Ok(())
}

pub fn get_execution(conn: &Connection, exec_id: i64) -> anyhow::Result<Option<WorkflowExecution>> {
    let mut stmt = conn.prepare(
        "SELECT id, workflow_id, inputs_json, status, current_step, started_at, finished_at, duration_ms
         FROM workflow_executions WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map([exec_id], |row| {
        Ok(WorkflowExecution {
            id: row.get(0)?,
            workflow_id: row.get(1)?,
            inputs_json: row.get(2)?,
            status: parse_status(&row.get::<_, String>(3)?),
            current_step: row.get(4)?,
            started_at: row.get(5)?,
            finished_at: row.get(6)?,
            duration_ms: row.get(7)?,
            step_results: std::collections::HashMap::new(),
        })
    })?;
    rows.next().transpose().map_err(Into::into)
}

fn parse_status(s: &str) -> ExecutionStatus {
    match s {
        "Pending" => ExecutionStatus::Pending,
        "Running" => ExecutionStatus::Running,
        "Completed" => ExecutionStatus::Completed,
        "Failed" => ExecutionStatus::Failed,
        "Cancelled" => ExecutionStatus::Cancelled,
        _ => ExecutionStatus::Pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WorkspaceRegistry;
    use std::collections::HashMap;

    fn dummy_wf() -> WorkflowDefinition {
        WorkflowDefinition {
            id: "test-wf".to_string(),
            name: "Test Workflow".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            inputs: vec![],
            outputs: vec![],
            steps: vec![],
            output_mapping: HashMap::new(),
        }
    }

    #[test]
    fn test_save_and_get_workflow() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let wf = dummy_wf();
        save_workflow(&conn, &wf).unwrap();
        let retrieved = get_workflow(&conn, "test-wf").unwrap().unwrap();
        assert_eq!(retrieved.id, "test-wf");
        assert_eq!(retrieved.name, "Test Workflow");
    }

    #[test]
    fn test_create_and_update_execution() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let wf = dummy_wf();
        save_workflow(&conn, &wf).unwrap();
        let exec_id = create_execution(&conn, "test-wf", r#"{"repo_path":"/tmp"}"#).unwrap();
        assert!(exec_id > 0);
        update_execution(&conn, exec_id, &ExecutionStatus::Running, Some("step1"), None).unwrap();
        let exec = get_execution(&conn, exec_id).unwrap().unwrap();
        assert_eq!(exec.status, ExecutionStatus::Running);
        assert_eq!(exec.current_step, Some("step1".to_string()));
    }

    #[test]
    fn test_end_to_end_workflow_lifecycle() {
        use crate::workflow::model::{ErrorPolicy, StepDefinition, StepType};
        use crate::workflow::{execute_workflow, validate_workflow};
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let wf = WorkflowDefinition {
            id: "e2e-wf".to_string(),
            name: "E2E Workflow".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            inputs: vec![],
            outputs: vec![],
            steps: vec![
                StepDefinition {
                    id: "step1".to_string(),
                    step_type: StepType::Skill { skill: "nonexistent-skill".to_string() },
                    inputs: HashMap::new(),
                    depends_on: vec![],
                    on_error: ErrorPolicy::Fail,
                    timeout_seconds: None,
                },
            ],
            output_mapping: HashMap::new(),
        };
        validate_workflow(&wf).unwrap();
        save_workflow(&conn, &wf).unwrap();

        let exec_id = create_execution(&conn, "e2e-wf", "{}").unwrap();
        update_execution(&conn, exec_id, &ExecutionStatus::Running, None, None).unwrap();

        // Execution should fail because skill does not exist
        let inputs: HashMap<String, String> = HashMap::new();
        let result = execute_workflow(&conn, &wf, inputs);
        assert!(result.is_err());

        update_execution(&conn, exec_id, &ExecutionStatus::Failed, Some("step1"), None).unwrap();
        let exec = get_execution(&conn, exec_id).unwrap().unwrap();
        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.current_step, Some("step1".to_string()));
    }
}
