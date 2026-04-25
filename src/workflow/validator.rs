use super::model::{StepDefinition, WorkflowDefinition};
use std::collections::{HashMap, HashSet};

/// Validate a workflow definition.
///
/// Checks:
/// 1. All step IDs are unique.
/// 2. All `depends_on` references exist.
/// 3. No circular dependencies.
/// 4. Output mappings reference valid steps.
pub fn validate_workflow(wf: &WorkflowDefinition) -> anyhow::Result<()> {
    let step_ids: HashSet<&str> = wf.steps.iter().map(|s| s.id.as_str()).collect();

    // 1. Unique step IDs
    if step_ids.len() != wf.steps.len() {
        return Err(anyhow::anyhow!("duplicate step ids detected"));
    }

    // 2. Valid depends_on references
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
    }

    // 3. No cycles
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
        if !visited.contains(id) {
            if dfs(id, &adj, &mut visited, &mut rec_stack) {
                return Err(anyhow::anyhow!("circular dependency detected in workflow"));
            }
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
    use crate::workflow::model::{ErrorPolicy, StepType};
    use std::collections::HashMap;

    fn dummy_step(id: &str, deps: Vec<&str>) -> StepDefinition {
        StepDefinition {
            id: id.to_string(),
            step_type: StepType::Skill {
                skill: "test".to_string(),
            },
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
}
