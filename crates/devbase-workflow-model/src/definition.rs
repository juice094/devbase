// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::step_type::StepType;

/// A workflow definition parsed from YAML.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct WorkflowDefinition {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(default)]
    pub inputs: Vec<WorkflowInputDef>,
    #[serde(default)]
    pub outputs: Vec<WorkflowOutputDef>,
    pub steps: Vec<StepDefinition>,
    #[serde(default, rename = "output_mapping")]
    pub output_mapping: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct WorkflowInputDef {
    pub name: String,
    #[serde(rename = "type", default = "default_string_type")]
    pub input_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<serde_yaml::Value>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct WorkflowOutputDef {
    pub name: String,
    #[serde(rename = "type", default = "default_string_type")]
    pub output_type: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StepDefinition {
    pub id: String,
    #[serde(flatten)]
    pub step_type: StepType,
    #[serde(default)]
    pub inputs: HashMap<String, serde_yaml::Value>,
    #[serde(default, rename = "depends_on")]
    pub depends_on: Vec<String>,
    #[serde(default, rename = "on_error")]
    pub on_error: ErrorPolicy,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub enum ErrorPolicy {
    #[default]
    #[serde(rename = "fail")]
    Fail,
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "retry")]
    Retry { count: u32, backoff_ms: u64 },
    #[serde(rename = "fallback")]
    Fallback { step_id: String },
}

pub fn default_string_type() -> String {
    "string".to_string()
}
