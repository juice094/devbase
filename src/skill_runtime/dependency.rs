use super::SkillRow;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet, VecDeque};

/// Resolve all transitive dependencies for a skill, returning them in
/// topological order (dependencies first, then the requested skill's direct deps).
pub fn resolve_dependencies(conn: &Connection, skill_id: &str) -> anyhow::Result<Vec<SkillRow>> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut order: Vec<String> = Vec::new();
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();

    // BFS to collect all transitive deps and build edge map
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(skill_id.to_string());
    visited.insert(skill_id.to_string());

    while let Some(current_id) = queue.pop_front() {
        let skill = match super::registry::get_skill(conn, &current_id)? {
            Some(s) => s,
            None => continue,
        };

        let deps: Vec<String> = skill.dependencies.iter().map(|d| d.id.clone()).collect();
        edges.insert(current_id.clone(), deps.clone());

        for dep_id in deps {
            if visited.insert(dep_id.clone()) {
                queue.push_back(dep_id);
            }
        }
    }

    // Detect cycles
    if let Some(cycle) = detect_cycle(skill_id, &edges) {
        return Err(anyhow::anyhow!("Dependency cycle detected: {}", cycle.join(" → ")));
    }

    // Build reverse adjacency list: dep -> [skills that depend on it]
    // and in_degree for each skill (how many deps it has)
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for id in &visited {
        in_degree.entry(id.clone()).or_insert(0);
    }
    for (skill, deps) in &edges {
        for dep in deps {
            if visited.contains(dep) {
                *in_degree.entry(skill.clone()).or_insert(0) += 1;
                adj.entry(dep.clone()).or_default().push(skill.clone());
            }
        }
    }

    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    while let Some(id) = queue.pop_front() {
        order.push(id.clone());
        if let Some(dependents) = adj.get(&id) {
            for dependent in dependents {
                if let Some(deg) = in_degree.get_mut(dependent) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }
    }

    if order.len() != visited.len() {
        return Err(anyhow::anyhow!("Dependency graph has unreachable nodes (possible cycle)"));
    }

    // Exclude the root skill itself; return only its dependencies in topological order
    let dep_rows: Vec<SkillRow> = order
        .into_iter()
        .filter(|id| id != skill_id)
        .filter_map(|id| super::registry::get_skill(conn, &id).ok().flatten())
        .collect();

    Ok(dep_rows)
}

/// Check if the dependency graph starting from `start` contains a cycle.
/// Returns the cycle path if found.
fn detect_cycle(start: &str, edges: &HashMap<String, Vec<String>>) -> Option<Vec<String>> {
    #[derive(Clone, Copy)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let mut color: HashMap<String, Color> = HashMap::new();
    let mut parent: HashMap<String, String> = HashMap::new();

    for id in edges.keys() {
        color.entry(id.clone()).or_insert(Color::White);
    }

    fn dfs(
        node: &str,
        edges: &HashMap<String, Vec<String>>,
        color: &mut HashMap<String, Color>,
        parent: &mut HashMap<String, String>,
    ) -> Option<Vec<String>> {
        color.insert(node.to_string(), Color::Gray);

        if let Some(neighbors) = edges.get(node) {
            for neighbor in neighbors {
                match color.get(neighbor).copied().unwrap_or(Color::White) {
                    Color::White => {
                        parent.insert(neighbor.clone(), node.to_string());
                        if let Some(cycle) = dfs(neighbor, edges, color, parent) {
                            return Some(cycle);
                        }
                    }
                    Color::Gray => {
                        // Cycle found — reconstruct path
                        let mut cycle = vec![neighbor.clone()];
                        let mut cur = node.to_string();
                        while cur != *neighbor {
                            cycle.push(cur.clone());
                            cur = parent.get(&cur).cloned().unwrap_or_default();
                        }
                        cycle.push(neighbor.clone());
                        cycle.reverse();
                        return Some(cycle);
                    }
                    Color::Black => {}
                }
            }
        }

        color.insert(node.to_string(), Color::Black);
        None
    }

    dfs(start, edges, &mut color, &mut parent)
}

/// Install missing dependencies for a skill.
///
/// Returns the list of installed dependency IDs.
pub fn install_missing_dependencies(
    conn: &Connection,
    skill: &super::SkillMeta,
    _git_base_url: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    let mut installed = Vec::new();

    for dep in &skill.dependencies {
        if super::registry::get_skill(conn, &dep.id)?.is_some() {
            continue; // already installed
        }

        // Try to install from explicit source
        if let Some(ref source) = dep.source {
            let _ = super::registry::install_skill_from_git(conn, source, Some(&dep.id))?;
            installed.push(dep.id.clone());
            continue;
        }

        // TODO: derive from git_base_url or a central registry
        // For now, report as missing
        return Err(anyhow::anyhow!(
            "Dependency '{}' of skill '{}' is not installed and has no source URL. \
             Install it manually with: devbase skill install <url> --id {}",
            dep.id,
            skill.id,
            dep.id
        ));
    }

    Ok(installed)
}

/// Validate that all declared dependencies of a skill are satisfied.
pub fn validate_dependencies(
    conn: &Connection,
    skill: &super::SkillMeta,
) -> anyhow::Result<Vec<String>> {
    let mut missing = Vec::new();
    for dep in &skill.dependencies {
        if super::registry::get_skill(conn, &dep.id)?.is_none() {
            missing.push(dep.id.clone());
        }
    }
    Ok(missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_runtime::{SkillDependency, SkillMeta, SkillType};

    fn test_skill(id: &str, deps: Vec<&str>) -> SkillMeta {
        SkillMeta {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            author: None,
            tags: vec![],
            entry_script: None,
            category: None,
            skill_type: SkillType::Builtin,
            local_path: std::path::PathBuf::from(format!("skills/{}", id)),
            inputs: vec![],
            outputs: vec![],
            dependencies: deps
                .into_iter()
                .map(|d| SkillDependency {
                    id: d.to_string(),
                    version: None,
                    source: None,
                })
                .collect(),
            embedding: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_used_at: None,
            body: String::new(),
        }
    }

    #[test]
    fn test_detect_cycle_direct() {
        let mut edges = HashMap::new();
        edges.insert("a".to_string(), vec!["b".to_string()]);
        edges.insert("b".to_string(), vec!["a".to_string()]);
        let cycle = detect_cycle("a", &edges);
        assert!(cycle.is_some());
        let c = cycle.unwrap();
        assert_eq!(c.first(), Some(&"a".to_string()));
        assert_eq!(c.last(), Some(&"a".to_string()));
    }

    #[test]
    fn test_detect_cycle_none() {
        let mut edges = HashMap::new();
        edges.insert("a".to_string(), vec!["b".to_string()]);
        edges.insert("b".to_string(), vec!["c".to_string()]);
        let cycle = detect_cycle("a", &edges);
        assert!(cycle.is_none());
    }

    #[test]
    fn test_resolve_topological_order() {
        let conn = crate::registry::WorkspaceRegistry::init_db().unwrap();
        let skills = vec![
            test_skill("a", vec!["b", "c"]),
            test_skill("b", vec!["c"]),
            test_skill("c", vec![]),
        ];
        for s in &skills {
            crate::skill_runtime::registry::install_skill(&conn, s).unwrap();
        }

        let resolved = resolve_dependencies(&conn, "a").unwrap();
        let ids: Vec<String> = resolved.iter().map(|s| s.id.clone()).collect();
        assert_eq!(ids, vec!["c", "b"]);
    }

    #[test]
    fn test_resolve_cycle_fails() {
        let conn = crate::registry::WorkspaceRegistry::init_db().unwrap();
        let skills = vec![test_skill("x", vec!["y"]), test_skill("y", vec!["x"])];
        for s in &skills {
            crate::skill_runtime::registry::install_skill(&conn, s).unwrap();
        }

        let err = resolve_dependencies(&conn, "x").unwrap_err();
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn test_validate_dependencies_missing() {
        let conn = crate::registry::WorkspaceRegistry::init_db().unwrap();
        let skill = test_skill("a", vec!["missing-dep"]);
        let missing = validate_dependencies(&conn, &skill).unwrap();
        assert_eq!(missing, vec!["missing-dep"]);
    }

    #[test]
    fn test_validate_dependencies_all_satisfied() {
        let conn = crate::registry::WorkspaceRegistry::init_db().unwrap();
        let dep = test_skill("dep1", vec![]);
        crate::skill_runtime::registry::install_skill(&conn, &dep).unwrap();
        let skill = test_skill("a", vec!["dep1"]);
        let missing = validate_dependencies(&conn, &skill).unwrap();
        assert!(missing.is_empty());
    }
}
