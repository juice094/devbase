use crate::registry::{OplogEntry, RepoEntry, RemoteEntry, WorkspaceRegistry};
use chrono::Utc;
use git2::Repository;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use walkdir::WalkDir;

pub async fn run_json(path: &str, register: bool) -> anyhow::Result<serde_json::Value> {
    let root = PathBuf::from(path);
    if !root.exists() {
        return Ok(serde_json::json!({
            "success": false,
            "count": 0,
            "registered": 0,
            "repos": [],
            "error": format!("Path does not exist: {}", path)
        }));
    }

    let config = crate::config::Config::load().unwrap_or_default();
    let repos = discover_repos(&root, Some(&config.github))?;
    let count = repos.len();

    let mut registered = 0usize;
    if register {
        info!("Registering {} repositories into local database", repos.len());
        let mut conn = WorkspaceRegistry::init_db()?;
        for repo in &repos {
            WorkspaceRegistry::save_repo(&mut conn, repo)?;
            if let Some(stars) = repo.stars {
                let _ = WorkspaceRegistry::save_stars_cache(&conn, &repo.id, stars);
            }
        }
        registered = repos.len();
    }

    let repo_json: Vec<serde_json::Value> = repos
        .iter()
        .map(|repo| {
            let primary = repo.primary_remote();
            serde_json::json!({
                "id": repo.id,
                "local_path": repo.local_path.to_string_lossy(),
                "upstream_url": primary.and_then(|r| r.upstream_url.clone()),
                "default_branch": primary.and_then(|r| r.default_branch.clone()),
                "tags": repo.tags.join(","),
                "language": repo.language
            })
        })
        .collect();

    // Log to oplog
    if let Ok(conn) = WorkspaceRegistry::init_db() {
        let _ = WorkspaceRegistry::save_oplog(
            &conn,
            &OplogEntry {
                id: None,
                operation: "scan".to_string(),
                repo_id: None,
                details: Some(format!("path={}, discovered={}, registered={}", path, count, registered)),
                status: "success".to_string(),
                timestamp: Utc::now(),
            },
        );
    }

    Ok(serde_json::json!({
        "success": true,
        "count": count,
        "registered": registered,
        "repos": repo_json
    }))
}

pub async fn run(path: &str, register: bool) -> anyhow::Result<()> {
    let result = run_json(path, register).await?;

    if !result["success"].as_bool().unwrap_or(false) {
        println!("{}", result["error"].as_str().unwrap_or("Unknown error"));
        return Ok(());
    }

    let count = result["count"].as_u64().unwrap_or(0) as usize;
    if count == 0 {
        println!("No workspaces found under {}", path);
        return Ok(());
    }

    println!("\nDiscovered {} workspace(s):\n", count);
    for repo in result["repos"].as_array().unwrap_or(&vec![]) {
        let id = repo["id"].as_str().unwrap_or("");
        let local_path = repo["local_path"].as_str().unwrap_or("");
        let upstream = repo["upstream_url"].as_str().unwrap_or("(none)");
        let branch = repo["default_branch"].as_str().unwrap_or("(unknown)");
        let language = repo["language"].as_str().unwrap_or("unknown");
        println!(
            "  [{}] {}\n         upstream: {}\n         branch: {}\n         language: {}",
            id, local_path, upstream, branch, language
        );
    }

    let registered = result["registered"].as_u64().unwrap_or(0);
    if registered > 0 {
        println!("\n✅ Registered {} workspace(s) to devbase database.", registered);
    } else {
        println!("\nℹ️  Use --register to persist these workspaces to the database.");
    }

    Ok(())
}

fn discover_repos(
    root: &Path,
    github: Option<&crate::config::GithubConfig>,
) -> anyhow::Result<Vec<RepoEntry>> {
    let mut git_repos = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() == ".git" && entry.file_type().is_dir() {
            let repo_path = entry.path().parent().unwrap_or(root).to_path_buf();

            // Skip nested .git inside submodules if possible
            if is_nested_submodule(&repo_path, &git_repos) {
                continue;
            }

            match inspect_repo(&repo_path, github) {
                Ok(repo) => git_repos.push(repo),
                Err(e) => warn!("Failed to inspect {}: {}", repo_path.display(), e),
            }
        }
    }

    // Discover non-git workspaces by marker files
    let mut non_git_repos = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let name = entry.file_name().to_string_lossy();
        let is_marker = (name == "SOUL.md" || name == "MEMORY.md" || name == ".devbase")
            && entry.file_type().is_file();
        if !is_marker {
            continue;
        }
        let ws_path = entry.path().parent().unwrap_or(root).to_path_buf();
        // Skip if already inside a known git repo
        if is_nested_submodule(&ws_path, &git_repos) {
            continue;
        }
        // Skip duplicates
        if non_git_repos.iter().any(|r: &RepoEntry| r.local_path == ws_path) {
            continue;
        }
        match inspect_non_git_workspace(&ws_path) {
            Ok(repo) => non_git_repos.push(repo),
            Err(e) => warn!("Failed to inspect non-git workspace {}: {}", ws_path.display(), e),
        }
    }

    let mut repos = git_repos;
    repos.extend(non_git_repos);
    Ok(repos)
}



pub fn detect_language(path: &Path) -> Option<String> {
    if path.join("Cargo.toml").exists() {
        Some("Rust".to_string())
    } else if path.join("package.json").exists() {
        Some("Node".to_string())
    } else if path.join("go.mod").exists() {
        Some("Go".to_string())
    } else if path.join("pyproject.toml").exists() || path.join("requirements.txt").exists() {
        Some("Python".to_string())
    } else if path.join("CMakeLists.txt").exists() {
        Some("C++".to_string())
    } else {
        None
    }
}

fn parse_github_owner_repo(upstream_url: &str) -> Option<(String, String)> {
    let url = upstream_url.trim_end_matches(".git");
    let path_part = if let Some(idx) = url.find("github.com/") {
        &url[idx + "github.com/".len()..]
    } else if let Some(idx) = url.find("github.com:") {
        &url[idx + "github.com:".len()..]
    } else {
        return None;
    };
    let parts: Vec<&str> = path_part.split('/').collect();
    if parts.len() < 2 {
        return None;
    }
    Some((parts[0].to_string(), parts[1].to_string()))
}

pub fn fetch_github_stars(
    upstream_url: &str,
    github: Option<&crate::config::GithubConfig>,
) -> Option<u64> {
    let (owner, repo) = parse_github_owner_repo(upstream_url)?;

    let timeout_secs = github.map(|g| g.timeout_seconds).unwrap_or(5);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .ok()?;
    let mut request = client
        .get(format!("https://api.github.com/repos/{}/{}", owner, repo))
        .header("User-Agent", "devbase-cli");
    if let Some(token) = github.and_then(|g| g.token.as_deref()) {
        request = request.header("Authorization", format!("Bearer {}", token));
    }
    let response = request.send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let json: serde_json::Value = response.json().ok()?;
    json.get("stargazers_count")?.as_u64()
}

pub fn inspect_repo(
    path: &Path,
    github: Option<&crate::config::GithubConfig>,
) -> anyhow::Result<RepoEntry> {
    let repo = Repository::open(path)?;

    let id = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let upstream_url = repo
        .remotes()
        .ok()
        .and_then(|remotes| remotes.get(0).map(String::from))
        .and_then(|name| repo.find_remote(&name).ok())
        .and_then(|remote| remote.url().map(String::from));

    let stars = upstream_url.as_deref().and_then(|u| fetch_github_stars(u, github));

    let default_branch = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(String::from));

    let language = detect_language(path);

    let tags = if id.ends_with("-main") || id.ends_with("-master") {
        vec![
            "discovered".to_string(),
            "zip-snapshot".to_string(),
            "needs-migration".to_string(),
        ]
    } else {
        vec!["discovered".to_string()]
    };

    let remote_entry = RemoteEntry {
        remote_name: "origin".to_string(),
        upstream_url,
        default_branch,
        last_sync: None,
    };

    Ok(RepoEntry {
        id,
        local_path: path.to_path_buf(),
        tags,
        discovered_at: Utc::now(),
        language,
        workspace_type: "git".to_string(),
        data_tier: "private".to_string(),
        last_synced_at: None,
        stars,
        remotes: vec![remote_entry],
    })
}

pub fn inspect_non_git_workspace(path: &Path) -> anyhow::Result<RepoEntry> {
    let id = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let language = detect_language(path);

    let workspace_type = if path.join("SOUL.md").exists()
        || path.join(".claude").is_dir()
    {
        "openclaw"
    } else {
        "generic"
    };

    Ok(RepoEntry {
        id,
        local_path: path.to_path_buf(),
        tags: vec!["discovered".to_string()],
        discovered_at: Utc::now(),
        language,
        workspace_type: workspace_type.to_string(),
        data_tier: "private".to_string(),
        last_synced_at: None,
        stars: None,
        remotes: Vec::new(),
    })
}

fn is_nested_submodule(path: &Path, found: &[RepoEntry]) -> bool {
    found.iter().any(|r| path.starts_with(&r.local_path) && path != r.local_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_language_rust() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(detect_language(dir.path()), Some("Rust".to_string()));
    }

    #[test]
    fn test_detect_language_node() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_language(dir.path()), Some("Node".to_string()));
    }

    #[test]
    fn test_detect_language_go() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("go.mod"), "module foo").unwrap();
        assert_eq!(detect_language(dir.path()), Some("Go".to_string()));
    }

    #[test]
    fn test_detect_language_python_pyproject() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[project]").unwrap();
        assert_eq!(detect_language(dir.path()), Some("Python".to_string()));
    }

    #[test]
    fn test_detect_language_python_requirements() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("requirements.txt"), "requests").unwrap();
        assert_eq!(detect_language(dir.path()), Some("Python".to_string()));
    }

    #[test]
    fn test_detect_language_cpp() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("CMakeLists.txt"), "cmake_minimum_required()").unwrap();
        assert_eq!(detect_language(dir.path()), Some("C++".to_string()));
    }

    #[test]
    fn test_detect_language_none() {
        let dir = TempDir::new().unwrap();
        assert_eq!(detect_language(dir.path()), None);
    }

    #[test]
    fn test_is_nested_submodule_true() {
        let parent = RepoEntry {
            id: "parent".to_string(),
            local_path: PathBuf::from("/workspace/parent"),
            tags: vec![],
            discovered_at: Utc::now(),
            language: None,
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };
        let child = Path::new("/workspace/parent/sub");
        assert!(is_nested_submodule(child, &[parent]));
    }

    #[test]
    fn test_is_nested_submodule_false() {
        let parent = RepoEntry {
            id: "parent".to_string(),
            local_path: PathBuf::from("/workspace/parent"),
            tags: vec![],
            discovered_at: Utc::now(),
            language: None,
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };
        let same = Path::new("/workspace/parent");
        assert!(!is_nested_submodule(same, &[parent.clone()]));

        let sibling = Path::new("/workspace/other");
        assert!(!is_nested_submodule(sibling, &[parent]));
    }

    #[test]
    fn test_zip_snapshot_tags_main() {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path().join("myproject-main");
        fs::create_dir(&repo_path).unwrap();
        git2::Repository::init(&repo_path).unwrap();

        let entry = inspect_repo(&repo_path, None).unwrap();
        assert_eq!(
            entry.tags,
            vec!["discovered", "zip-snapshot", "needs-migration"]
        );
    }

    #[test]
    fn test_zip_snapshot_tags_master() {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path().join("myproject-master");
        fs::create_dir(&repo_path).unwrap();
        git2::Repository::init(&repo_path).unwrap();

        let entry = inspect_repo(&repo_path, None).unwrap();
        assert_eq!(
            entry.tags,
            vec!["discovered", "zip-snapshot", "needs-migration"]
        );
    }

    #[test]
    fn test_normal_tags() {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path().join("myproject");
        fs::create_dir(&repo_path).unwrap();
        git2::Repository::init(&repo_path).unwrap();

        let entry = inspect_repo(&repo_path, None).unwrap();
        assert_eq!(entry.tags, vec!["discovered"]);
    }

    #[test]
    fn test_inspect_non_git_workspace_generic() {
        let dir = TempDir::new().unwrap();
        let ws_path = dir.path().join("notes");
        fs::create_dir(&ws_path).unwrap();
        fs::write(ws_path.join(".devbase"), "").unwrap();

        let entry = inspect_non_git_workspace(&ws_path).unwrap();
        assert_eq!(entry.id, "notes");
        assert_eq!(entry.workspace_type, "generic");
        assert!(entry.remotes.is_empty());
    }

    #[test]
    fn test_inspect_non_git_workspace_openclaw() {
        let dir = TempDir::new().unwrap();
        let ws_path = dir.path().join("claw");
        fs::create_dir(&ws_path).unwrap();
        fs::write(ws_path.join("SOUL.md"), "# soul").unwrap();

        let entry = inspect_non_git_workspace(&ws_path).unwrap();
        assert_eq!(entry.workspace_type, "openclaw");
    }

    #[test]
    fn test_discover_repos_finds_non_git_workspaces() {
        let dir = TempDir::new().unwrap();
        let git_path = dir.path().join("gitrepo");
        fs::create_dir(&git_path).unwrap();
        git2::Repository::init(&git_path).unwrap();

        let generic_path = dir.path().join("genericws");
        fs::create_dir(&generic_path).unwrap();
        fs::write(generic_path.join("MEMORY.md"), "# memory").unwrap();

        let repos = discover_repos(dir.path(), None).unwrap();
        assert_eq!(repos.len(), 2);

        let types: std::collections::HashSet<_> = repos.iter().map(|r| r.workspace_type.as_str()).collect();
        assert!(types.contains("git"));
        assert!(types.contains("generic"));
    }

    #[test]
    fn test_parse_github_owner_repo_https() {
        assert_eq!(
            parse_github_owner_repo("https://github.com/rust-lang/rust.git"),
            Some(("rust-lang".to_string(), "rust".to_string()))
        );
        assert_eq!(
            parse_github_owner_repo("https://github.com/rust-lang/rust"),
            Some(("rust-lang".to_string(), "rust".to_string()))
        );
    }

    #[test]
    fn test_parse_github_owner_repo_ssh() {
        assert_eq!(
            parse_github_owner_repo("git@github.com:rust-lang/rust.git"),
            Some(("rust-lang".to_string(), "rust".to_string()))
        );
        assert_eq!(
            parse_github_owner_repo("git@github.com:rust-lang/rust"),
            Some(("rust-lang".to_string(), "rust".to_string()))
        );
    }

    #[test]
    fn test_parse_github_owner_repo_non_github() {
        assert_eq!(
            parse_github_owner_repo("https://gitlab.com/rust-lang/rust.git"),
            None
        );
        assert_eq!(
            parse_github_owner_repo("https://bitbucket.org/rust-lang/rust.git"),
            None
        );
    }

    #[test]
    fn test_parse_github_owner_repo_invalid() {
        assert_eq!(parse_github_owner_repo("not-a-url"), None);
        assert_eq!(parse_github_owner_repo("https://github.com/short"), None);
    }
}
