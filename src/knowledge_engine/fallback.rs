use std::collections::HashMap;
use std::path::Path;

pub fn generate_fallback_summary(path: &Path) -> (String, String) {
    // Try language-specific metadata files
    if let Some((summary, keywords)) = try_cargo_toml(path) {
        return (summary, keywords);
    }
    if let Some((summary, keywords)) = try_package_json(path) {
        return (summary, keywords);
    }
    if let Some((summary, keywords)) = try_go_mod(path) {
        return (summary, keywords);
    }
    if let Some((summary, keywords)) = try_pyproject(path) {
        return (summary, keywords);
    }

    // Last resort: file type distribution
    let mut counts: HashMap<String, usize> = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                *counts.entry(ext.to_lowercase()).or_insert(0) += 1;
            }
        }
    }
    let mut pairs: Vec<(String, usize)> = counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1));
    let top: Vec<String> =
        pairs.into_iter().take(3).map(|(e, c)| format!("{}({})", e, c)).collect();
    if top.is_empty() {
        ("Unclassified project".to_string(), "unknown".to_string())
    } else {
        (format!("Project containing files: {}", top.join(", ")), top.join(", "))
    }
}

pub(crate) fn try_cargo_toml(path: &Path) -> Option<(String, String)> {
    let cargo_path = path.join("Cargo.toml");
    let cargo = std::fs::read_to_string(&cargo_path).ok()?;
    let value: toml::Value = toml::from_str(&cargo).ok()?;
    let package = value.get("package")?;
    let desc = package.get("description")?.as_str()?;
    let name = package.get("name")?.as_str()?;
    let workspace_members: Vec<String> = value
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let summary = if workspace_members.len() > 1 {
        format!(
            "Rust workspace '{}' with {} crates: {}",
            name,
            workspace_members.len(),
            workspace_members.join(", ")
        )
    } else {
        format!("Rust crate '{}' - {}", name, desc)
    };
    let keywords =
        format!("rust, {}, {}", name, workspace_members.first().cloned().unwrap_or_default());
    Some((summary, keywords))
}

pub(crate) fn try_package_json(path: &Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path.join("package.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let name = value.get("name")?.as_str()?;
    let desc = value.get("description")?.as_str().unwrap_or("Node.js project");
    let summary = format!("Node project '{}' - {}", name, desc);
    Some((summary, format!("node, {}, javascript", name)))
}

pub(crate) fn try_go_mod(path: &Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path.join("go.mod")).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("module ") {
            let module_name = line.strip_prefix("module ")?.trim();
            let summary = format!("Go module '{}'", module_name);
            return Some((summary, format!("go, {}", module_name)));
        }
    }
    None
}

pub(crate) fn try_pyproject(path: &Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path.join("pyproject.toml")).ok()?;
    let value: toml::Value = toml::from_str(&content).ok()?;
    let project = value.get("project")?;
    let name = project.get("name")?.as_str()?;
    let desc = project.get("description")?.as_str().unwrap_or("Python project");
    let summary = format!("Python project '{}' - {}", name, desc);
    Some((
        summary,
        format!("python, {}, {}", name, desc.split_whitespace().next().unwrap_or("")),
    ))
}

