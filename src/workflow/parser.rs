use super::model::WorkflowDefinition;
use std::path::Path;

/// Parse a workflow definition from a YAML file.
pub fn parse_workflow_yaml(path: &Path) -> anyhow::Result<WorkflowDefinition> {
    let content = std::fs::read_to_string(path)?;
    let def: WorkflowDefinition = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse workflow YAML: {e}"))?;
    validate_workflow_id(&def.id)?;
    Ok(def)
}

/// Parse from a YAML string (useful for tests and inline definitions).
pub fn parse_workflow_yaml_str(content: &str) -> anyhow::Result<WorkflowDefinition> {
    let def: WorkflowDefinition = serde_yaml::from_str(content)
        .map_err(|e| anyhow::anyhow!("Failed to parse workflow YAML: {e}"))?;
    validate_workflow_id(&def.id)?;
    Ok(def)
}

fn validate_workflow_id(id: &str) -> anyhow::Result<()> {
    if id.is_empty() {
        return Err(anyhow::anyhow!("workflow id must not be empty"));
    }
    if id.contains(' ') {
        return Err(anyhow::anyhow!("workflow id must not contain spaces: '{id}'"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_workflow() {
        let yaml = r#"
id: test-pipeline
name: Test Pipeline
version: 0.1.0
description: A test workflow
inputs:
  - name: repo_path
    type: string
    required: true
steps:
  - id: lint
    skill: rust-clippy
    inputs:
      path: "${inputs.repo_path}"
    on_error: fail
  - id: test
    skill: cargo-test
    depends_on: [lint]
    inputs:
      path: "${inputs.repo_path}"
output_mapping:
  result: "${steps.test.outputs.stdout}"
"#;
        let wf = parse_workflow_yaml_str(yaml).unwrap();
        assert_eq!(wf.id, "test-pipeline");
        assert_eq!(wf.steps.len(), 2);
        assert_eq!(wf.steps[0].id, "lint");
        assert_eq!(wf.steps[1].depends_on, vec!["lint"]);
    }

    #[test]
    fn test_parse_invalid_id() {
        let yaml = r#"
id: "bad id"
name: Bad
version: 0.1.0
steps: []
"#;
        assert!(parse_workflow_yaml_str(yaml).is_err());
    }
}
