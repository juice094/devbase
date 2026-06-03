// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::clients::WorkflowClient;
use crate::mcp::McpTool;
use anyhow::Context;

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
- Audit workflow inventory before creating new workflows

Do NOT use this for:
- Running a workflow (use devkit_workflow_run instead)
- Checking workflow execution status (use devkit_workflow_status instead)
- Discovering skills (use devkit_skill_list instead)

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
        ctx.list_workflows()
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
                    "workflow_id": { "type": "string", "description": "ID of the workflow to run (from devkit_workflow_list)" },
                    "inputs": { "type": "object", "description": "Optional JSON object of input key-value pairs" }
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
        let workflow_id = args
            .get("workflow_id")
            .and_then(|v| v.as_str())
            .context("workflow_id is required")?
            .to_string();
        let inputs_value = args.get("inputs").cloned().unwrap_or(serde_json::json!({}));

        if workflow_id.is_empty() {
            anyhow::bail!("workflow_id must not be empty");
        }

        ctx.run_workflow(&workflow_id, inputs_value)
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
        let exec_id = args
            .get("execution_id")
            .and_then(|v| v.as_i64())
            .context("execution_id is required")?;

        if exec_id <= 0 {
            anyhow::bail!("execution_id must be a positive integer");
        }

        ctx.get_execution(exec_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::model::{ErrorPolicy, StepDefinition, StepType, WorkflowDefinition};
    use std::collections::HashMap;

    fn dag_workflow(id: &str, steps: Vec<StepDefinition>) -> WorkflowDefinition {
        WorkflowDefinition {
            id: id.to_string(),
            name: id.to_string(),
            version: "0.1.0".to_string(),
            description: None,
            inputs: vec![],
            outputs: vec![],
            steps,
            output_mapping: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_workflow_list_empty_registry() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();

        let tool = DevkitWorkflowListTool;
        let result = tool.invoke(serde_json::json!({}), &mut ctx).await.unwrap();
        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(result.get("count").and_then(|v| v.as_u64()), Some(0));
    }

    #[tokio::test]
    async fn test_workflow_run_not_found() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();

        let tool = DevkitWorkflowRunTool;
        let result = tool
            .invoke(serde_json::json!({"workflow_id": "nonexistent-wf"}), &mut ctx)
            .await
            .unwrap();
        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(false));
    }

    #[tokio::test]
    async fn test_workflow_status_invalid_id() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();

        let tool = DevkitWorkflowStatusTool;
        let result = tool.invoke(serde_json::json!({"execution_id": -1}), &mut ctx).await;
        assert!(result.is_err(), "negative execution_id should return an error");
    }

    /// End-to-end: 3-step DAG workflow (a -> b -> c) executed via MCP tool.
    /// Verifies registration -> run -> status query full chain.
    #[tokio::test]
    async fn test_workflow_run_dag_success() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();

        let wf = dag_workflow(
            "dag-success",
            vec![
                StepDefinition {
                    id: "a".to_string(),
                    step_type: StepType::Condition { r#if: "true".to_string() },
                    inputs: HashMap::new(),
                    depends_on: vec![],
                    on_error: ErrorPolicy::Fail,
                    timeout_seconds: None,
                },
                StepDefinition {
                    id: "b".to_string(),
                    step_type: StepType::Condition { r#if: "true".to_string() },
                    inputs: HashMap::new(),
                    depends_on: vec!["a".to_string()],
                    on_error: ErrorPolicy::Fail,
                    timeout_seconds: None,
                },
                StepDefinition {
                    id: "c".to_string(),
                    step_type: StepType::Condition { r#if: "true".to_string() },
                    inputs: HashMap::new(),
                    depends_on: vec!["b".to_string()],
                    on_error: ErrorPolicy::Fail,
                    timeout_seconds: None,
                },
            ],
        );
        {
            let conn = ctx.conn().unwrap();
            crate::workflow::save_workflow(&conn, &wf).unwrap();
        }

        let run_tool = DevkitWorkflowRunTool;
        let result = run_tool
            .invoke(serde_json::json!({"workflow_id": "dag-success"}), &mut ctx)
            .await
            .unwrap();

        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(result.get("status").and_then(|v| v.as_str()), Some("Completed"));

        let exec_id = result.get("execution_id").and_then(|v| v.as_i64()).unwrap();
        assert!(exec_id > 0);

        let step_results = result.get("step_results").and_then(|v| v.as_object()).unwrap();
        assert!(step_results.contains_key("a"));
        assert!(step_results.contains_key("b"));
        assert!(step_results.contains_key("c"));

        // Round-trip status query
        let status_tool = DevkitWorkflowStatusTool;
        let status = status_tool
            .invoke(serde_json::json!({"execution_id": exec_id}), &mut ctx)
            .await
            .unwrap();
        assert_eq!(status.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(status.get("status").and_then(|v| v.as_str()), Some("Completed"));
        assert_eq!(status.get("workflow_id").and_then(|v| v.as_str()), Some("dag-success"));
    }

    /// Failure propagation: step a succeeds, step b fails (missing skill) with ErrorPolicy::Fail.
    /// Verifies workflow returns Failed status and execution record reflects failure.
    #[tokio::test]
    async fn test_workflow_run_failure_propagation() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();

        let wf = dag_workflow(
            "dag-fail",
            vec![
                StepDefinition {
                    id: "a".to_string(),
                    step_type: StepType::Condition { r#if: "true".to_string() },
                    inputs: HashMap::new(),
                    depends_on: vec![],
                    on_error: ErrorPolicy::Fail,
                    timeout_seconds: None,
                },
                StepDefinition {
                    id: "b".to_string(),
                    step_type: StepType::Skill {
                        skill: "nonexistent-skill".to_string(),
                    },
                    inputs: HashMap::new(),
                    depends_on: vec!["a".to_string()],
                    on_error: ErrorPolicy::Fail,
                    timeout_seconds: None,
                },
            ],
        );
        {
            let conn = ctx.conn().unwrap();
            crate::workflow::save_workflow(&conn, &wf).unwrap();
        }

        let run_tool = DevkitWorkflowRunTool;
        let result = run_tool
            .invoke(serde_json::json!({"workflow_id": "dag-fail"}), &mut ctx)
            .await
            .unwrap();

        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(result.get("status").and_then(|v| v.as_str()), Some("Failed"));

        let exec_id = result.get("execution_id").and_then(|v| v.as_i64()).unwrap();
        assert!(exec_id > 0);

        let status_tool = DevkitWorkflowStatusTool;
        let status = status_tool
            .invoke(serde_json::json!({"execution_id": exec_id}), &mut ctx)
            .await
            .unwrap();
        assert_eq!(status.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(status.get("status").and_then(|v| v.as_str()), Some("Failed"));
    }
}
