// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

/// Result of a skill execution.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionResult {
    pub skill_id: String,
    pub status: ExecutionStatus,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Timeout,
}

impl ExecutionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExecutionStatus::Pending => "pending",
            ExecutionStatus::Running => "running",
            ExecutionStatus::Success => "success",
            ExecutionStatus::Failed => "failed",
            ExecutionStatus::Timeout => "timeout",
        }
    }
}

impl std::str::FromStr for ExecutionStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(ExecutionStatus::Pending),
            "running" => Ok(ExecutionStatus::Running),
            "success" => Ok(ExecutionStatus::Success),
            "failed" => Ok(ExecutionStatus::Failed),
            "timeout" => Ok(ExecutionStatus::Timeout),
            _ => Err(anyhow::anyhow!("unknown execution status: {}", s)),
        }
    }
}
