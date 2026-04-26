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

    #[test]
    fn test_apply_overrides() {
        let mut repo = crate::registry::RepoEntry {
            id: "devbase".to_string(),
            local_path: std::path::PathBuf::from("/tmp/devbase"),
            tags: vec!["old".to_string()],
            language: Some("rust".to_string()),
            discovered_at: chrono::Utc::now(),
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };
        let overrides = RepoOverride {
            path: "devbase".to_string(),
            tags: vec!["rust".to_string(), "cli".to_string()],
            tier: Some("hot".to_string()),
            workspace_type: Some("rust".to_string()),
        };
        apply_overrides(&mut repo, &overrides);
        assert_eq!(repo.tags, vec!["rust", "cli"]);
        assert_eq!(repo.data_tier, "hot");
        assert_eq!(repo.workspace_type, "rust");
    }
}
