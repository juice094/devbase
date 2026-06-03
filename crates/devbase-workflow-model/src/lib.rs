// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

pub mod definition;
pub mod execution;
pub mod step_type;

// Re-export all public items so that `use devbase_workflow_model::*` continues to work.
pub use definition::*;
pub use execution::*;
pub use step_type::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_loop_serde_roundtrip() {
        let step = definition::StepDefinition {
            id: "loop1".to_string(),
            step_type: step_type::StepType::Loop {
                for_each: "${inputs.repos}".to_string(),
                body: vec![definition::StepDefinition {
                    id: "lint".to_string(),
                    step_type: step_type::StepType::Skill { skill: "clippy".to_string() },
                    inputs: HashMap::new(),
                    depends_on: vec![],
                    on_error: definition::ErrorPolicy::Fail,
                    timeout_seconds: None,
                }],
            },
            inputs: HashMap::new(),
            depends_on: vec![],
            on_error: definition::ErrorPolicy::Fail,
            timeout_seconds: None,
        };
        let yaml = serde_yaml::to_string(&step).unwrap();
        let parsed: definition::StepDefinition = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(step, parsed);
    }

    #[test]
    fn test_loop_serde_backward_compat() {
        // Old YAML without 'body' field should parse with empty body
        let yaml = r#"
id: loop1
for_each: "repo-a,repo-b"
"#;
        let parsed: definition::StepDefinition = serde_yaml::from_str(yaml).unwrap();
        match &parsed.step_type {
            step_type::StepType::Loop { for_each, body } => {
                assert_eq!(for_each, "repo-a,repo-b");
                assert!(body.is_empty());
            }
            _ => panic!("expected Loop step"),
        }
    }
}
