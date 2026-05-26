// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecution {
    pub id: i64,
    pub workflow_id: String,
    pub inputs_json: String,
    pub status: ExecutionStatus,
    pub current_step: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub step_results: HashMap<String, StepResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub status: ExecutionStatus,
    pub outputs: HashMap<String, serde_json::Value>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error: Option<String>,
}
