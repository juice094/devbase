use super::parser::parse_skill_md;
use std::path::Path;

/// Validate a skill directory for publishing.
///
/// Checks:
/// 1. SKILL.md exists and is parseable
/// 2. Required fields present (name, version, description)
/// 3. Git repository exists
/// 4. Working tree is clean (no uncommitted changes)
pub fn validate_skill_for_publish(path: &Path) -> anyhow::Result<PublishValidation> {
    let skill_md = path.join("SKILL.md");
    if !skill_md.exists() {
        return Err(anyhow::anyhow!("SKILL.md not found in {}", path.display()));
    }

    let skill = parse_skill_md(&skill_md)?;

    if skill.name.is_empty() {
        return Err(anyhow::anyhow!("SKILL.md missing required field: name"));
    }
    if skill.version.is_empty() {
        return Err(anyhow::anyhow!("SKILL.md missing required field: version"));
    }
    if skill.description.is_empty() {
        return Err(anyhow::anyhow!("SKILL.md missing required field: description"));
    }

    // Check git repository
    let git_dir = path.join(".git");
    let is_git_repo = git_dir.exists() || git_dir.is_symlink();

    let git_status = if is_git_repo {
        match check_git_status(path) {
            Ok(status) => Some(status),
            Err(e) => {
                return Err(anyhow::anyhow!("Git status check failed: {}", e));
            }
        }
    } else {
        None
    };

    Ok(PublishValidation {
        skill_id: skill.id,
        name: skill.name,
        version: skill.version,
        description: skill.description,
        is_git_repo,
        git_clean: git_status.as_ref().map(|s| s.is_clean).unwrap_or(false),
        git_branch: git_status.as_ref().map(|s| s.branch.clone()),
        git_ahead: git_status.as_ref().map(|s| s.ahead).unwrap_or(0),
    })
}

#[derive(Debug, Clone)]
pub struct PublishValidation {
    pub skill_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub is_git_repo: bool,
    pub git_clean: bool,
    pub git_branch: Option<String>,
    pub git_ahead: i32,
}

#[derive(Debug, Clone)]
struct GitStatus {
    is_clean: bool,
    branch: String,
    ahead: i32,
}

fn check_git_status(path: &Path) -> anyhow::Result<GitStatus> {
    let repo = git2::Repository::open(path)?;

    // Check for uncommitted changes
    let mut status_opts = git2::StatusOptions::new();
    status_opts.include_untracked(true);
    let statuses = repo.statuses(Some(&mut status_opts))?;
    let is_clean = statuses.is_empty();

    // Get current branch
    let head = repo.head()?;
    let branch = head
        .shorthand()
        .unwrap_or("HEAD")
        .to_string();

    Ok(GitStatus {
        is_clean,
        branch,
        ahead: 0,
    })
}

/// Create a git tag for the skill version.
pub fn create_version_tag(path: &Path, tag: &str, message: &str) -> anyhow::Result<()> {
    let repo = git2::Repository::open(path)?;
    let sig = repo.signature()?;
    let head = repo.head()?;
    let target = head.peel_to_commit()?;
    repo.tag(
        tag,
        target.as_object(),
        &sig,
        message,
        false,
    )?;
    Ok(())
}

/// Push a git tag to the default remote.
///
/// Uses the default remote name (typically "origin"). If no remote is
/// configured, returns an error with a helpful message.
pub fn push_tag_to_remote(path: &Path, tag: &str) -> anyhow::Result<()> {
    let repo = git2::Repository::open(path)?;
    let remote_name = get_default_remote(&repo)?;
    let mut remote = repo.find_remote(&remote_name)?;

    let refspec = format!("refs/tags/{}:refs/tags/{}", tag, tag);

    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, allowed_types| {
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
        } else {
            Err(git2::Error::from_str(
                "authentication required but no supported credential method available",
            ))
        }
    });

    let mut push_opts = git2::PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    remote.push(&[&refspec], Some(&mut push_opts)).map_err(|e| {
        let msg = format_push_error(&e);
        anyhow::anyhow!("Failed to push tag '{}': {}", tag, msg)
    })?;

    Ok(())
}

fn get_default_remote(repo: &git2::Repository) -> anyhow::Result<String> {
    if repo.find_remote("origin").is_ok() {
        return Ok("origin".to_string());
    }

    let remotes = repo.remotes()?;
    if let Some(first) = remotes.iter().flatten().next() {
        return Ok(first.to_string());
    }

    Err(anyhow::anyhow!(
        "No remote configured. Add a remote with: git remote add origin <url>"
    ))
}

fn format_push_error(e: &git2::Error) -> String {
    let class = e.class();
    let message = e.message();

    if class == git2::ErrorClass::Ssh {
        format!(
            "SSH authentication failed. Ensure your SSH key is added to the agent. ({})",
            message
        )
    } else if class == git2::ErrorClass::Http {
        format!(
            "HTTP authentication failed. Check your credentials or remote URL. ({})",
            message
        )
    } else if class == git2::ErrorClass::Net {
        format!(
            "Network error — check your internet connection. ({})",
            message
        )
    } else if message.contains("not found") || message.contains("does not exist") {
        format!(
            "Remote reference not found. Ensure the remote URL is correct. ({})",
            message
        )
    } else {
        format!("{} (git error class: {:?})", message, class)
    }
}

/// Check whether the `gh` CLI is installed on this system.
pub fn has_gh_cli() -> bool {
    which::which("gh").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn init_repo(path: &Path) {
        fs::create_dir_all(path).unwrap();
        let output = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(path)
            .output()
            .expect("git init failed");
        assert!(output.status.success(), "git init failed: {:?}", output);

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(path)
            .output()
            .unwrap();

        // Create initial commit
        fs::write(path.join("file.txt"), "hello").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial", "--quiet"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    #[test]
    fn test_push_tag_no_remote() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_path = tmp.path().join("skill");
        init_repo(&repo_path);

        create_version_tag(&repo_path, "v1.0.0", "test").unwrap();

        let err = push_tag_to_remote(&repo_path, "v1.0.0").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("No remote configured"),
            "Expected 'No remote configured' in: {}",
            msg
        );
    }

    #[test]
    fn test_push_tag_success_to_bare_remote() {
        let tmp = tempfile::tempdir().unwrap();
        let remote_path = tmp.path().join("remote.git");
        let repo_path = tmp.path().join("skill");

        // Create bare remote
        fs::create_dir_all(&remote_path).unwrap();
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .current_dir(&remote_path)
            .output()
            .unwrap();

        // Create local repo
        init_repo(&repo_path);

        // Determine default branch name
        let branch = {
            let repo = git2::Repository::open(&repo_path).unwrap();
            let head = repo.head().unwrap();
            head.shorthand().unwrap_or("master").to_string()
        };

        // Add remote and push branch so the commit exists on remote
        Command::new("git")
            .args(["remote", "add", "origin", remote_path.to_str().unwrap()])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["push", "origin", &format!("HEAD:refs/heads/{}", branch)])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        create_version_tag(&repo_path, "v1.0.0", "test").unwrap();
        push_tag_to_remote(&repo_path, "v1.0.0").unwrap();

        // Verify tag exists on remote
        let output = Command::new("git")
            .args(["tag", "-l", "v1.0.0"])
            .current_dir(&remote_path)
            .output()
            .unwrap();
        let tags = String::from_utf8_lossy(&output.stdout);
        assert!(tags.contains("v1.0.0"), "Tag should exist on remote");
    }

    #[test]
    fn test_get_default_remote_origin() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_path = tmp.path().join("repo");
        init_repo(&repo_path);

        Command::new("git")
            .args(["remote", "add", "origin", "https://example.com/repo.git"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let repo = git2::Repository::open(&repo_path).unwrap();
        assert_eq!(get_default_remote(&repo).unwrap(), "origin");
    }

    #[test]
    fn test_get_default_remote_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_path = tmp.path().join("repo");
        init_repo(&repo_path);

        Command::new("git")
            .args(["remote", "add", "upstream", "https://example.com/repo.git"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let repo = git2::Repository::open(&repo_path).unwrap();
        assert_eq!(get_default_remote(&repo).unwrap(), "upstream");
    }
}
