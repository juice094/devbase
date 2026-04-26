use super::model::{StepDefinition, WorkflowDefinition};
use std::collections::{HashMap, HashSet, VecDeque};

/// A batch of steps that can be executed in parallel.
pub type ExecutionBatch = Vec<StepDefinition>;

/// Build a topological schedule: each inner vec is a batch of steps with no
/// inter-dependencies, so they may run in parallel.
pub fn build_schedule(wf: &WorkflowDefinition) -> anyhow::Result<Vec<ExecutionBatch>> {
    let n = wf.steps.len();
    let mut in_degree: HashMap<&str, usize> = wf.steps.iter().map(|s| (s.id.as_str(), 0)).collect();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for step in &wf.steps {
        for dep in &step.depends_on {
            *in_degree.get_mut(step.id.as_str()).expect("step id initialized in in_degree") += 1;
            adj.entry(dep.as_str()).or_default().push(step.id.as_str());
        }
    }

    let mut queue: VecDeque<&str> =
        in_degree.iter().filter(|(_, d)| **d == 0).map(|(id, _)| *id).collect();

    let mut batches: Vec<ExecutionBatch> = Vec::new();
    let mut processed = 0;

    while !queue.is_empty() {
        let batch_size = queue.len();
        let mut batch = Vec::with_capacity(batch_size);
        let mut next_queue: VecDeque<&str> = VecDeque::new();

        for _ in 0..batch_size {
            let id = queue.pop_front().expect("queue not empty: checked by while condition");
            let step = wf.steps.iter().find(|s| s.id == id).expect("step id must exist").clone();
            batch.push(step);
            processed += 1;

            if let Some(children) = adj.get(id) {
                for &child in children {
                    let deg = in_degree.get_mut(child).expect("child id initialized in in_degree");
                    *deg -= 1;
                    if *deg == 0 {
                        next_queue.push_back(child);
                    }
                }
            }
        }

        batches.push(batch);
        queue = next_queue;
    }

    if processed != n {
        return Err(anyhow::anyhow!(
            "scheduler could not process all steps ({processed}/{n}); cycle or orphaned step"
        ));
    }

    Ok(batches)
}

/// Returns the set of step ids that are directly or indirectly depended upon by `target_id`.
pub fn transitive_deps(wf: &WorkflowDefinition, target_id: &str) -> HashSet<String> {
    let mut visited = HashSet::new();
    let mut stack = vec![target_id.to_string()];
    let step_map: HashMap<&str, &StepDefinition> =
        wf.steps.iter().map(|s| (s.id.as_str(), s)).collect();

    while let Some(current) = stack.pop() {
        if visited.insert(current.clone())
            && let Some(step) = step_map.get(current.as_str())
        {
            for dep in &step.depends_on {
                stack.push(dep.clone());
            }
        }
    }
    visited
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::model::{ErrorPolicy, StepType};
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
    fn test_linear_schedule() {
        let wf = dummy_wf(vec![
            dummy_step("a", vec![]),
            dummy_step("b", vec!["a"]),
            dummy_step("c", vec!["b"]),
        ]);
        let batches = build_schedule(&wf).unwrap();
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0][0].id, "a");
        assert_eq!(batches[1][0].id, "b");
        assert_eq!(batches[2][0].id, "c");
    }

    #[test]
    fn test_parallel_schedule() {
        let wf = dummy_wf(vec![
            dummy_step("a", vec![]),
            dummy_step("b", vec!["a"]),
            dummy_step("c", vec!["a"]),
            dummy_step("d", vec!["b", "c"]),
        ]);
        let batches = build_schedule(&wf).unwrap();
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 1); // a
        assert_eq!(batches[1].len(), 2); // b, c
        assert_eq!(batches[2].len(), 1); // d
    }

    #[test]
    fn test_transitive_deps() {
        let wf = dummy_wf(vec![
            dummy_step("a", vec![]),
            dummy_step("b", vec!["a"]),
            dummy_step("c", vec!["b"]),
            dummy_step("d", vec!["a", "c"]),
        ]);
        let deps = transitive_deps(&wf, "d");
        assert!(deps.contains("a"));
        assert!(deps.contains("b"));
        assert!(deps.contains("c"));
        assert!(deps.contains("d"));
        assert_eq!(deps.len(), 4);
    }

    #[test]
    fn test_transitive_deps_leaf() {
        let wf = dummy_wf(vec![dummy_step("a", vec![]), dummy_step("b", vec!["a"])]);
        let deps = transitive_deps(&wf, "a");
        assert_eq!(deps, std::collections::HashSet::from(["a".to_string()]));
    }
}
