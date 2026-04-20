use serde::Deserialize;

/// Static configuration override for a repository.
/// Allows users to declare tags, tier, and workspace_type in `workspace/repos.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct RepoOverride {
    pub path: String,
    pub tags: Vec<String>,
    pub tier: Option<String>,
    pub workspace_type: Option<String>,
}

/// Root structure of `repos.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct ReposToml {
    #[serde(rename = "repo")]
    pub repos: Vec<RepoOverride>,
}

/// Load `workspace/repos.toml` if it exists.
pub fn load_repos_toml() -> Option<ReposToml> {
    let ws = crate::registry::WorkspaceRegistry::workspace_dir().ok()?;
    let path = ws.join("repos.toml");
    let content = std::fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}

/// Apply static overrides to a `RepoEntry` when the path matches.
pub fn apply_overrides(repo: &mut crate::registry::RepoEntry, overrides: &RepoOverride) {
    if !overrides.tags.is_empty() {
        repo.tags = overrides.tags.clone();
    }
    if let Some(tier) = &overrides.tier {
        repo.data_tier = tier.clone();
    }
    if let Some(ws_type) = &overrides.workspace_type {
        repo.workspace_type = ws_type.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repos_toml() {
        let toml_str = r#"
[[repo]]
path = "devbase"
tags = ["rust", "cli"]
tier = "hot"
workspace_type = "rust"
"#;
        let parsed: ReposToml = toml::from_str(toml_str).unwrap();
        assert_eq!(parsed.repos.len(), 1);
        assert_eq!(parsed.repos[0].path, "devbase");
        assert_eq!(parsed.repos[0].tags, vec!["rust", "cli"]);
        assert_eq!(parsed.repos[0].tier, Some("hot".to_string()));
    }
}
