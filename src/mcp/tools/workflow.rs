use crate::mcp::McpTool;
use std::collections::HashMap;

#[derive(Clone)]
pub struct DevkitWorkflowListTool;

impl McpTool for DevkitWorkflowListTool {
    fn name(&self) -> &'static str {
        "devkit_workflow_list"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"List registered workflows in the devbase registry.

Use this when the user wants to:
- See what automation workflows are available
- Choose a workflow to run
- Audit workflow inventory

Parameters: none

Returns: JSON array of workflows with id, name, and version."#,
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        })
    }

    async fn invoke(
        &self,
        _args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = ctx.conn()?;
        let workflows = crate::workflow::state::list_workflows(&conn)?;
        let items: Vec<serde_json::Value> = workflows
            .into_iter()
            .map(|(id, name, version)| {
                serde_json::json!({
                    "id": id,
                    "name": name,
                    "version": version
                })
            })
            .collect();
        Ok(serde_json::json!({
            "success": true,
            "count": items.len(),
            "workflows": items
        }))
    }
}

#[derive(Clone)]
pub struct DevkitWorkflowRunTool;

impl McpTool for DevkitWorkflowRunTool {
    fn name(&self) -> &'static str {
        "devkit_workflow_run"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Execute a registered workflow by ID.

Use this when the user wants to:
- Run an automation workflow (e.g., index all repos, generate a report)
- Trigger a skill sequence or multi-step pipeline
- Execute batch operations defined as workflows

Parameters:
- workflow_id: ID of the workflow to run (from devkit_workflow_list)
- inputs: Optional JSON object of input key-value pairs

Returns: execution summary with status, step results, and execution_id."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workflow_id": { "type": "string" },
                    "inputs": { "type": "object" }
                },
                "required": ["workflow_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let workflow_id =
            args.get("workflow_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let inputs_value = args.get("inputs").cloned().unwrap_or(serde_json::json!({}));

        if workflow_id.is_empty() {
            return Ok(serde_json::json!({
                "success": false,
                "error": "workflow_id is required"
            }));
        }

        let conn = ctx.conn()?;
        let wf = match crate::workflow::state::get_workflow(&conn, &workflow_id)? {
            Some(wf) => wf,
            None => {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": format!("workflow '{}' not found", workflow_id)
                }));
            }
        };

        // Parse inputs into HashMap<String, String>
        let inputs: HashMap<String, String> = if let Some(obj) = inputs_value.as_object() {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        } else {
            HashMap::new()
        };

        let inputs_json = inputs_value.to_string();
        let exec_id = crate::workflow::state::create_execution(&conn, &workflow_id, &inputs_json)?;
        crate::workflow::state::update_execution(
            &conn,
            exec_id,
            &crate::workflow::model::ExecutionStatus::Running,
            None,
            None,
        )?;

        let pool = ctx.pool();
        let start = std::time::Instant::now();
        let result = crate::workflow::executor::execute_workflow(&conn, &pool, &wf, inputs);
        let duration_ms = start.elapsed().as_millis() as i64;

        match result {
            Ok(step_results) => {
                crate::workflow::state::update_execution(
                    &conn,
                    exec_id,
                    &crate::workflow::model::ExecutionStatus::Completed,
                    None,
                    Some(duration_ms),
                )?;
                let results_json: HashMap<String, serde_json::Value> = step_results
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::to_value(v).unwrap_or(serde_json::json!(null))))
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
                crate::workflow::state::update_execution(
                    &conn,
                    exec_id,
                    &crate::workflow::model::ExecutionStatus::Failed,
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
}

#[derive(Clone)]
pub struct DevkitWorkflowStatusTool;

impl McpTool for DevkitWorkflowStatusTool {
    fn name(&self) -> &'static str {
        "devkit_workflow_status"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query the status of a workflow execution.

Use this when the user wants to:
- Check if a previously started workflow has finished
- Debug a failed workflow execution
- Monitor long-running automation pipelines

Parameters:
- execution_id: The execution ID returned by devkit_workflow_run

Returns: execution record with status, current_step, timestamps, and duration."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "execution_id": { "type": "integer" }
                },
                "required": ["execution_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let exec_id = args.get("execution_id").and_then(|v| v.as_i64()).unwrap_or(0);

        if exec_id <= 0 {
            return Ok(serde_json::json!({
                "success": false,
                "error": "execution_id must be a positive integer"
            }));
        }

        let conn = ctx.conn()?;
        match crate::workflow::state::get_execution(&conn, exec_id)? {
            Some(exec) => Ok(serde_json::json!({
                "success": true,
                "execution_id": exec.id,
                "workflow_id": exec.workflow_id,
                "status": format!("{:?}", exec.status),
                "current_step": exec.current_step,
                "started_at": exec.started_at,
                "finished_at": exec.finished_at,
                "duration_ms": exec.duration_ms,
                "inputs": exec.inputs_json
            })),
            None => Ok(serde_json::json!({
                "success": false,
                "error": format!("execution {} not found", exec_id)
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_list_empty_registry() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
        }
        let mut ctx = crate::storage::AppContext::with_defaults().unwrap();

        let tool = DevkitWorkflowListTool;
        let result = tool.invoke(serde_json::json!({}), &mut ctx).await.unwrap();
        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(result.get("count").and_then(|v| v.as_u64()), Some(0));
    }

    #[tokio::test]
    async fn test_workflow_run_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
        }
        let mut ctx = crate::storage::AppContext::with_defaults().unwrap();

        let tool = DevkitWorkflowRunTool;
        let result = tool
            .invoke(serde_json::json!({"workflow_id": "nonexistent-wf"}), &mut ctx)
            .await
            .unwrap();
        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(false));
    }

    #[tokio::test]
    async fn test_workflow_status_invalid_id() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
        }
        let mut ctx = crate::storage::AppContext::with_defaults().unwrap();

        let tool = DevkitWorkflowStatusTool;
        let result = tool.invoke(serde_json::json!({"execution_id": -1}), &mut ctx).await.unwrap();
        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(false));
    }
}
