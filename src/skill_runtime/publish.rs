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
