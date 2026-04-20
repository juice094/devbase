use crate::registry::RepoEntry;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Discovery {
    pub from: String,
    pub to: String,
    pub relation_type: String,
    pub confidence: f64,
    pub description: String,
}

pub fn discover_dependencies(repos: &[RepoEntry]) -> Vec<Discovery> {
    let mut discoveries = Vec::new();

    for repo in repos {
        let cargo_toml = repo.local_path.join("Cargo.toml");
        if cargo_toml.exists()
            && let Ok(content) = std::fs::read_to_string(&cargo_toml)
            && let Ok(value) = content.parse::<toml::Value>()
        {
            let mut dep_names = HashSet::new();

            if let Some(deps) = value.get("dependencies").and_then(|d| d.as_table()) {
                for name in deps.keys() {
                    dep_names.insert(normalize_dep_name(name));
                }
            }

            if let Some(ws) = value.get("workspace").and_then(|w| w.as_table())
                && let Some(ws_deps) = ws.get("dependencies").and_then(|d| d.as_table())
            {
                for name in ws_deps.keys() {
                    dep_names.insert(normalize_dep_name(name));
                }
            }

            for other in repos {
                if other.id == repo.id {
                    continue;
                }
                let normalized_other = normalize_dep_name(&other.id);
                if dep_names.contains(&normalized_other) {
                    discoveries.push(Discovery {
                        from: repo.id.clone(),
                        to: other.id.clone(),
                        relation_type: "depends_on".to_string(),
                        confidence: 0.9,
                        description: format!(
                            "{} depends on crate '{}' from {}",
                            repo.id, other.id, other.id
                        ),
                    });
                }
            }
        }

        let package_json = repo.local_path.join("package.json");
        if package_json.exists()
            && let Ok(content) = std::fs::read_to_string(&package_json)
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&content)
        {
            let mut dep_names = HashSet::new();
            if let Some(deps) = value.get("dependencies").and_then(|d| d.as_object()) {
                for name in deps.keys() {
                    dep_names.insert(normalize_dep_name(name));
                }
            }
            if let Some(dev_deps) = value.get("devDependencies").and_then(|d| d.as_object()) {
                for name in dev_deps.keys() {
                    dep_names.insert(normalize_dep_name(name));
                }
            }
            for other in repos {
                if other.id == repo.id {
                    continue;
                }
                let normalized_other = normalize_dep_name(&other.id);
                if dep_names.contains(&normalized_other) {
                    discoveries.push(Discovery {
                        from: repo.id.clone(),
                        to: other.id.clone(),
                        relation_type: "depends_on".to_string(),
                        confidence: 0.85,
                        description: format!(
                            "{} depends on npm package '{}' from {}",
                            repo.id, other.id, other.id
                        ),
                    });
                }
            }
        }

        let go_mod = repo.local_path.join("go.mod");
        if go_mod.exists()
            && let Ok(content) = std::fs::read_to_string(&go_mod)
        {
            let mut dep_names = HashSet::new();
            let mut in_require_block = false;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("require (") {
                    in_require_block = true;
                    continue;
                }
                if in_require_block && trimmed.starts_with(')') {
                    in_require_block = false;
                    continue;
                }
                if in_require_block {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if !parts.is_empty() {
                        dep_names.insert(parts[0].to_string());
                    }
                    continue;
                }
                if trimmed.starts_with("require ") && !trimmed.contains('(') {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        dep_names.insert(parts[1].to_string());
                    }
                }
            }
            for other in repos {
                if other.id == repo.id {
                    continue;
                }
                if dep_names.contains(&other.id) {
                    discoveries.push(Discovery {
                        from: repo.id.clone(),
                        to: other.id.clone(),
                        relation_type: "depends_on".to_string(),
                        confidence: 0.85,
                        description: format!(
                            "{} depends on go module '{}' from {}",
                            repo.id, other.id, other.id
                        ),
                    });
                }
            }
        }
    }

    discoveries
}

fn normalize_dep_name(name: &str) -> String {
    name.replace('_', "-").to_lowercase()
}

pub fn discover_similar_projects(conn: &rusqlite::Connection) -> anyhow::Result<Vec<Discovery>> {
    let mut stmt = conn.prepare("SELECT repo_id, keywords FROM repo_summaries")?;
    let rows =
        stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)))?;

    let mut keywords_map: HashMap<String, HashSet<String>> = HashMap::new();
    for row in rows {
        let (repo_id, keywords_opt) = row?;
        if let Some(keywords) = keywords_opt {
            let set: HashSet<String> = keywords
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if !set.is_empty() {
                keywords_map.insert(repo_id, set);
            }
        }
    }

    let repo_ids: Vec<String> = keywords_map.keys().cloned().collect();
    let mut discoveries = Vec::new();

    for i in 0..repo_ids.len() {
        for j in (i + 1)..repo_ids.len() {
            let a = &repo_ids[i];
            let b = &repo_ids[j];
            let set_a = keywords_map.get(a).unwrap();
            let set_b = keywords_map.get(b).unwrap();

            let intersection: HashSet<String> = set_a.intersection(set_b).cloned().collect();
            if intersection.is_empty() {
                continue;
            }

            let union: HashSet<String> = set_a.union(set_b).cloned().collect();
            let jaccard = intersection.len() as f64 / union.len() as f64;

            if jaccard > 0.0 {
                let shared: Vec<String> = intersection.into_iter().collect();
                discoveries.push(Discovery {
                    from: a.clone(),
                    to: b.clone(),
                    relation_type: "similar_to".to_string(),
                    confidence: jaccard,
                    description: format!("{} and {} share keywords: {:?}", a, b, shared),
                });
            }
        }
    }

    discoveries.sort_by(|a, b| {
        b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(discoveries)
}
