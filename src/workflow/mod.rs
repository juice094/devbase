// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
pub mod executor;
pub mod interpolate;
pub mod model;
pub mod parser;
pub mod scheduler;
pub mod state;
pub mod validator;

pub use executor::execute_workflow;
pub use model::*;
pub use parser::{parse_workflow_yaml, parse_workflow_yaml_str};
pub use scheduler::build_schedule;
pub use state::{
    create_execution, delete_workflow, get_execution, get_workflow, list_workflows, save_workflow,
    update_execution,
};
pub use validator::validate_workflow;

use crate::storage::AppContext;

impl crate::clients::WorkflowClient for AppContext {
    fn list_workflows(&self) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let workflows = state::list_workflows(&conn)?;
        let items: Vec<serde_json::Value> = workflows
            .into_iter()
            .map(|(id, name, version)| {
                serde_json::json!({"id": id, "name": name, "version": version})
            })
            .collect();
        Ok(serde_json::json!({"success": true, "count": items.len(), "workflows": items}))
    }

    fn get_workflow(&self, workflow_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        match state::get_workflow(&conn, workflow_id)? {
            Some(wf) => Ok(serde_json::json!({
                "success": true,
                "id": wf.id,
                "name": wf.name,
                "version": wf.version,
                "description": wf.description,
                "steps": wf.steps.len(),
            })),
            None => Ok(serde_json::json!({"success": false, "error": "workflow not found"})),
        }
    }

    fn run_workflow(
        &self,
        workflow_id: &str,
        inputs: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let wf = match state::get_workflow(&conn, workflow_id)? {
            Some(wf) => wf,
            None => {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": format!("workflow '{}' not found", workflow_id)
                }));
            }
        };

        let inputs_map: std::collections::HashMap<String, String> =
            if let Some(obj) = inputs.as_object() {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            } else {
                std::collections::HashMap::new()
            };

        let inputs_json = inputs.to_string();
        let exec_id = state::create_execution(&conn, workflow_id, &inputs_json)?;
        state::update_execution(&conn, exec_id, &model::ExecutionStatus::Running, None, None)?;

        let pool = self.pool();
        let start = std::time::Instant::now();
        let result = executor::execute_workflow(&conn, &pool, &wf, inputs_map);
        let duration_ms = start.elapsed().as_millis() as i64;

        match result {
            Ok(step_results) => {
                state::update_execution(
                    &conn,
                    exec_id,
                    &model::ExecutionStatus::Completed,
                    None,
                    Some(duration_ms),
                )?;
                let results_json: std::collections::HashMap<String, serde_json::Value> =
                    step_results
                        .into_iter()
                        .map(|(k, v)| {
                            (k, serde_json::to_value(v).unwrap_or(serde_json::json!(null)))
                        })
                        .collect();
                Ok(serde_json::json!({
                    "success": true,
                    "execution_id": exec_id,
                    "workflow_id": workflow_id,
                    "status": "Completed",
                    "duration_ms": duration_ms,
                    "step_results": results_json
                }))
            }
            Err(e) => {
                state::update_execution(
                    &conn,
                    exec_id,
                    &model::ExecutionStatus::Failed,
                    None,
                    Some(duration_ms),
                )?;
                Ok(serde_json::json!({
                    "success": false,
                    "execution_id": exec_id,
                    "workflow_id": workflow_id,
                    "status": "Failed",
                    "duration_ms": duration_ms,
                    "error": e.to_string()
                }))
            }
        }
    }

    fn get_execution(&self, exec_id: i64) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        match state::get_execution(&conn, exec_id)? {
            Some(exec) => Ok(serde_json::json!({
                "success": true,
                "execution_id": exec.id,
                "workflow_id": exec.workflow_id,
                "status": format!("{:?}", exec.status),
                "current_step": exec.current_step,
                "started_at": exec.started_at,
                "finished_at": exec.finished_at,
                "duration_ms": exec.duration_ms,
                "inputs": exec.inputs_json,
            })),
            None => Ok(serde_json::json!({
                "success": false,
                "error": format!("execution {} not found", exec_id)
            })),
        }
    }
}
