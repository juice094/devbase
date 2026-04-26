use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug, Clone, PartialEq)]
pub enum StepType {
    Skill {
        skill: String,
    },
    Subworkflow {
        workflow: String,
    },
    Parallel {
        parallel: Vec<StepDefinition>,
    },
    Condition {
        r#if: String,
    },
    Loop {
        for_each: String,
        body: Vec<StepDefinition>,
    },
}

impl serde::Serialize for StepType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        match self {
            StepType::Skill { skill } => {
                map.serialize_entry("type", "skill")?;
                map.serialize_entry("skill", skill)?;
            }
            StepType::Subworkflow { workflow } => {
                map.serialize_entry("type", "subworkflow")?;
                map.serialize_entry("workflow", workflow)?;
            }
            StepType::Parallel { parallel } => {
                map.serialize_entry("type", "parallel")?;
                map.serialize_entry("parallel", parallel)?;
            }
            StepType::Condition { r#if } => {
                map.serialize_entry("type", "condition")?;
                map.serialize_entry("if", r#if)?;
            }
            StepType::Loop { for_each, body } => {
                map.serialize_entry("type", "loop")?;
                map.serialize_entry("for_each", for_each)?;
                if !body.is_empty() {
                    map.serialize_entry("body", body)?;
                }
            }
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for StepType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_yaml::Value::deserialize(deserializer)?;
        let map = value
            .as_mapping()
            .ok_or_else(|| serde::de::Error::custom("step must be a mapping"))?;

        // Prefer explicit 'type' for future-proof extensibility
        if let Some(type_val) = map.get("type") {
            let type_str = type_val
                .as_str()
                .ok_or_else(|| serde::de::Error::custom("step 'type' must be a string"))?;
            return match type_str {
                "skill" => {
                    let skill = map.get("skill").and_then(|v| v.as_str()).ok_or_else(|| {
                        serde::de::Error::custom("skill step requires 'skill' field")
                    })?;
                    Ok(StepType::Skill { skill: skill.to_string() })
                }
                "subworkflow" | "workflow" => {
                    let workflow =
                        map.get("workflow").and_then(|v| v.as_str()).ok_or_else(|| {
                            serde::de::Error::custom("subworkflow step requires 'workflow' field")
                        })?;
                    Ok(StepType::Subworkflow { workflow: workflow.to_string() })
                }
                "parallel" => {
                    let parallel = map.get("parallel").ok_or_else(|| {
                        serde::de::Error::custom("parallel step requires 'parallel' field")
                    })?;
                    let parallel: Vec<StepDefinition> = serde_yaml::from_value(parallel.clone())
                        .map_err(|e| {
                            serde::de::Error::custom(format!("invalid parallel steps: {}", e))
                        })?;
                    Ok(StepType::Parallel { parallel })
                }
                "condition" | "if" => {
                    let r#if = map.get("if").and_then(|v| v.as_str()).ok_or_else(|| {
                        serde::de::Error::custom("condition step requires 'if' field")
                    })?;
                    Ok(StepType::Condition { r#if: r#if.to_string() })
                }
                "loop" | "for_each" => {
                    let for_each =
                        map.get("for_each").and_then(|v| v.as_str()).ok_or_else(|| {
                            serde::de::Error::custom("loop step requires 'for_each' field")
                        })?;
                    let body = map
                        .get("body")
                        .map(|v| serde_yaml::from_value::<Vec<StepDefinition>>(v.clone()))
                        .transpose()
                        .map_err(|e| serde::de::Error::custom(format!("invalid loop body: {}", e)))?
                        .unwrap_or_default();
                    Ok(StepType::Loop {
                        for_each: for_each.to_string(),
                        body,
                    })
                }
                _ => Err(serde::de::Error::custom(format!("unknown step type: '{}'", type_str))),
            };
        }

        // Backward-compatible field-name inference (legacy YAML without 'type')
        if map.contains_key("skill") {
            let skill = map
                .get("skill")
                .and_then(|v| v.as_str())
                .ok_or_else(|| serde::de::Error::custom("skill step requires 'skill' string"))?;
            return Ok(StepType::Skill { skill: skill.to_string() });
        }
        if map.contains_key("workflow") {
            let workflow = map.get("workflow").and_then(|v| v.as_str()).ok_or_else(|| {
                serde::de::Error::custom("subworkflow step requires 'workflow' string")
            })?;
            return Ok(StepType::Subworkflow { workflow: workflow.to_string() });
        }
        if map.contains_key("parallel") {
            let parallel = map.get("parallel").ok_or_else(|| {
                serde::de::Error::custom("parallel step requires 'parallel' field")
            })?;
            let parallel: Vec<StepDefinition> = serde_yaml::from_value(parallel.clone())
                .map_err(|e| serde::de::Error::custom(format!("invalid parallel steps: {}", e)))?;
            return Ok(StepType::Parallel { parallel });
        }
        if map.contains_key("if") {
            let r#if = map
                .get("if")
                .and_then(|v| v.as_str())
                .ok_or_else(|| serde::de::Error::custom("condition step requires 'if' string"))?;
            return Ok(StepType::Condition { r#if: r#if.to_string() });
        }
        if map.contains_key("for_each") {
            let for_each = map
                .get("for_each")
                .and_then(|v| v.as_str())
                .ok_or_else(|| serde::de::Error::custom("loop step requires 'for_each' string"))?;
            let body = map
                .get("body")
                .map(|v| serde_yaml::from_value::<Vec<StepDefinition>>(v.clone()))
                .transpose()
                .map_err(|e| serde::de::Error::custom(format!("invalid loop body: {}", e)))?
                .unwrap_or_default();
            return Ok(StepType::Loop {
                for_each: for_each.to_string(),
                body,
            });
        }

        Err(serde::de::Error::custom(
            "cannot infer step type: missing known fields (skill, workflow, parallel, if, for_each) or explicit 'type'",
        ))
    }
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

// Runtime state models

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

fn default_string_type() -> String {
    "string".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_loop_serde_roundtrip() {
        let step = StepDefinition {
            id: "loop1".to_string(),
            step_type: StepType::Loop {
                for_each: "${inputs.repos}".to_string(),
                body: vec![StepDefinition {
                    id: "lint".to_string(),
                    step_type: StepType::Skill { skill: "clippy".to_string() },
                    inputs: HashMap::new(),
                    depends_on: vec![],
                    on_error: ErrorPolicy::Fail,
                    timeout_seconds: None,
                }],
            },
            inputs: HashMap::new(),
            depends_on: vec![],
            on_error: ErrorPolicy::Fail,
            timeout_seconds: None,
        };
        let yaml = serde_yaml::to_string(&step).unwrap();
        let parsed: StepDefinition = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(step, parsed);
    }

    #[test]
    fn test_loop_serde_backward_compat() {
        // Old YAML without 'body' field should parse with empty body
        let yaml = r#"
id: loop1
for_each: "repo-a,repo-b"
"#;
        let parsed: StepDefinition = serde_yaml::from_str(yaml).unwrap();
        match &parsed.step_type {
            StepType::Loop { for_each, body } => {
                assert_eq!(for_each, "repo-a,repo-b");
                assert!(body.is_empty());
            }
            _ => panic!("expected Loop step"),
        }
    }
}
