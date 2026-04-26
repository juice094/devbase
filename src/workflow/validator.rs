use super::model::WorkflowDefinition;
use std::collections::{HashMap, HashSet};

/// Validate a workflow definition.
///
/// Checks:
/// 1. All step IDs are unique (including inside loop bodies).
/// 2. All `depends_on` references exist.
/// 3. No circular dependencies.
/// 4. Output mappings reference valid steps.
pub fn validate_workflow(wf: &WorkflowDefinition) -> anyhow::Result<()> {
    let step_ids: HashSet<&str> = wf.steps.iter().map(|s| s.id.as_str()).collect();

    // 1. Unique step IDs
    if step_ids.len() != wf.steps.len() {
        return Err(anyhow::anyhow!("duplicate step ids detected"));
    }

    // Collect loop body step IDs and validate they don't clash with global IDs
    let mut body_step_ids: HashSet<&str> = HashSet::new();
    for step in &wf.steps {
        if let super::model::StepType::Loop { body, .. } = &step.step_type {
            for body_step in body {
                if step_ids.contains(body_step.id.as_str()) {
                    return Err(anyhow::anyhow!(
                        "loop body step '{}' duplicates global step id",
                        body_step.id
                    ));
                }
                if !body_step_ids.insert(body_step.id.as_str()) {
                    return Err(anyhow::anyhow!(
                        "duplicate step id '{}' inside loop body",
                        body_step.id
                    ));
                }
            }
        }
    }

    // 2. Valid depends_on references
    // Loop body steps may depend on other body steps or global steps.
    let all_valid_ids: HashSet<&str> = step_ids.union(&body_step_ids).copied().collect();
    for step in &wf.steps {
        for dep in &step.depends_on {
            if !step_ids.contains(dep.as_str()) {
                return Err(anyhow::anyhow!(
                    "step '{}' depends on non-existent step '{}'",
                    step.id,
                    dep
                ));
            }
        }
        if let super::model::StepType::Loop { body, .. } = &step.step_type {
            for body_step in body {
                for dep in &body_step.depends_on {
                    if !all_valid_ids.contains(dep.as_str()) {
                        return Err(anyhow::anyhow!(
                            "loop body step '{}' depends on non-existent step '{}'",
                            body_step.id,
                            dep
                        ));
                    }
                }
            }
        }
    }

    // 3. No cycles (global DAG only; loop body is executed sequentially by executor)
    detect_cycle(wf)?;

    // 4. Output mapping validity
    for (out_key, template) in &wf.output_mapping {
        // Simple check: extract ${steps.X.outputs.Y} patterns
        for cap in extract_step_refs(template) {
            if !step_ids.contains(cap.as_str()) {
                return Err(anyhow::anyhow!(
                    "output_mapping key '{out_key}' references non-existent step '{cap}'"
                ));
            }
        }
    }

    Ok(())
}

fn detect_cycle(wf: &WorkflowDefinition) -> anyhow::Result<()> {
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let adj: HashMap<&str, Vec<&str>> = wf
        .steps
        .iter()
        .map(|s| (s.id.as_str(), s.depends_on.iter().map(|d| d.as_str()).collect()))
        .collect();

    for step in &wf.steps {
        let id = step.id.as_str();
        if !visited.contains(id) && dfs(id, &adj, &mut visited, &mut rec_stack) {
            return Err(anyhow::anyhow!("circular dependency detected in workflow"));
        }
    }
    Ok(())
}

fn dfs<'a>(
    node: &'a str,
    adj: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    rec_stack: &mut HashSet<&'a str>,
) -> bool {
    visited.insert(node);
    rec_stack.insert(node);
    if let Some(deps) = adj.get(node) {
        for &dep in deps {
            if !visited.contains(dep) {
                if dfs(dep, adj, visited, rec_stack) {
                    return true;
                }
            } else if rec_stack.contains(dep) {
                return true;
            }
        }
    }
    rec_stack.remove(node);
    false
}

fn extract_step_refs(template: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for cap in template.match_indices("${steps.") {
        let start = cap.0 + cap.1.len();
        if let Some(end) = template[start..].find(".") {
            refs.push(template[start..start + end].to_string());
        }
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::model::{ErrorPolicy, StepDefinition, StepType};
    use std::collections::HashMap;

    fn dummy_step(id: &str, deps: Vec<&str>) -> StepDefinition {
        StepDefinition {
            id: id.to_string(),
            step_type: StepType::Skill { skill: "test".to_string() },
            inputs: HashMap::new(),
            depends_on: deps.into_iter().map(|s| s.to_string()).collect(),
            on_error: ErrorPolicy::Fail,
            timeout_seconds: None,
        }
    }

    fn dummy_wf(steps: Vec<StepDefinition>) -> WorkflowDefinition {
        WorkflowDefinition {
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            inputs: vec![],
            outputs: vec![],
            steps,
            output_mapping: HashMap::new(),
        }
    }

    #[test]
    fn test_valid_dag() {
        let wf = dummy_wf(vec![
            dummy_step("a", vec![]),
            dummy_step("b", vec!["a"]),
            dummy_step("c", vec!["a"]),
            dummy_step("d", vec!["b", "c"]),
        ]);
        assert!(validate_workflow(&wf).is_ok());
    }

    #[test]
    fn test_cycle_detected() {
        let wf = dummy_wf(vec![
            dummy_step("a", vec!["c"]),
            dummy_step("b", vec!["a"]),
            dummy_step("c", vec!["b"]),
        ]);
        assert!(validate_workflow(&wf).is_err());
    }

    #[test]
    fn test_missing_dep() {
        let wf = dummy_wf(vec![dummy_step("a", vec!["missing"])]);
        assert!(validate_workflow(&wf).is_err());
    }

    #[test]
    fn test_loop_body_duplicate_global_id() {
        let wf = dummy_wf(vec![
            StepDefinition {
                id: "lint".to_string(),
                step_type: StepType::Skill { skill: "test".to_string() },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            },
            StepDefinition {
                id: "loop1".to_string(),
                step_type: StepType::Loop {
                    for_each: "a,b".to_string(),
                    body: vec![StepDefinition {
                        id: "lint".to_string(),
                        step_type: StepType::Skill { skill: "test".to_string() },
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
            },
        ]);
        assert!(validate_workflow(&wf).is_err());
    }

    #[test]
    fn test_loop_body_missing_dep() {
        let wf = dummy_wf(vec![StepDefinition {
            id: "loop1".to_string(),
            step_type: StepType::Loop {
                for_each: "a,b".to_string(),
                body: vec![StepDefinition {
                    id: "inner".to_string(),
                    step_type: StepType::Skill { skill: "test".to_string() },
                    inputs: HashMap::new(),
                    depends_on: vec!["missing".to_string()],
                    on_error: ErrorPolicy::Fail,
                    timeout_seconds: None,
                }],
            },
            inputs: HashMap::new(),
            depends_on: vec![],
            on_error: ErrorPolicy::Fail,
            timeout_seconds: None,
        }]);
        assert!(validate_workflow(&wf).is_err());
    }

    #[test]
    fn test_loop_body_valid() {
        let wf = dummy_wf(vec![
            StepDefinition {
                id: "setup".to_string(),
                step_type: StepType::Skill { skill: "test".to_string() },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            },
            StepDefinition {
                id: "loop1".to_string(),
                step_type: StepType::Loop {
                    for_each: "a,b".to_string(),
                    body: vec![StepDefinition {
                        id: "inner".to_string(),
                        step_type: StepType::Skill { skill: "test".to_string() },
                        inputs: HashMap::new(),
                        depends_on: vec!["setup".to_string()],
                        on_error: ErrorPolicy::Fail,
                        timeout_seconds: None,
                    }],
                },
                inputs: HashMap::new(),
                depends_on: vec!["setup".to_string()],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            },
        ]);
        assert!(validate_workflow(&wf).is_ok());
    }
}
