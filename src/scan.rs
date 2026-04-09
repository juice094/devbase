use crate::registry::{RepoEntry, RemoteEntry, WorkspaceRegistry};
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

    let repos = discover_repos(&root)?;
    let count = repos.len();

    let mut registered = 0usize;
    if register {
        info!("Registering {} repositories into local database", repos.len());
        let mut conn = WorkspaceRegistry::init_db()?;
        for repo in &repos {
            WorkspaceRegistry::save_repo(&mut conn, repo)?;
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
        println!("No Git repositories found under {}", path);
        return Ok(());
    }

    println!("\nDiscovered {} Git repository(es):\n", count);
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
        println!("\n✅ Registered {} repositories to devbase database.", registered);
    } else {
        println!("\nℹ️  Use --register to persist these repositories to the database.");
    }

    Ok(())
}

fn discover_repos(root: &Path) -> anyhow::Result<Vec<RepoEntry>> {
    let mut repos = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() == ".git" && entry.file_type().is_dir() {
            let repo_path = entry.path().parent().unwrap_or(root).to_path_buf();

            // Skip nested .git inside submodules if possible
            if is_nested_submodule(&repo_path, &repos) {
                continue;
            }

            match inspect_repo(&repo_path) {
                Ok(repo) => repos.push(repo),
                Err(e) => warn!("Failed to inspect {}: {}", repo_path.display(), e),
            }
        }
    }

    Ok(repos)
}

fn detect_language(path: &Path) -> Option<String> {
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

fn inspect_repo(path: &Path) -> anyhow::Result<RepoEntry> {
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
        remotes: vec![remote_entry],
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

        let entry = inspect_repo(&repo_path).unwrap();
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

        let entry = inspect_repo(&repo_path).unwrap();
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

        let entry = inspect_repo(&repo_path).unwrap();
        assert_eq!(entry.tags, vec!["discovered"]);
    }
}
