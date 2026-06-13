use std::path::Path;

use async_trait::async_trait;
use chrono::Utc;

use super::{SkillMeta, SkillType};

/// Trait for pluggable external skill sources.
/// Each implementation knows how to fetch skills from a specific origin
/// (GitHub repo, HTTP endpoint, local directory, etc.).
#[async_trait]
pub trait SkillSource: Send + Sync {
    /// Human-readable name for this source (e.g., "github:anthropics/skills").
    fn name(&self) -> &str;

    /// Fetch skills from this source. Called by the sync pipeline.
    async fn fetch(&self) -> anyhow::Result<Vec<SkillMeta>>;
}

// ── GitHub Source ──────────────────────────────────────────────────

pub struct GitHubSource {
    pub owner: String,
    pub repo: String,
    /// Path within the repo to scan for SKILL.md files (e.g., "skills").
    pub path: String,
    client: reqwest::Client,
}

impl GitHubSource {
    pub fn new(owner: &str, repo: &str, path: &str) -> Self {
        GitHubSource {
            owner: owner.to_string(),
            repo: repo.to_string(),
            path: path.to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SkillSource for GitHubSource {
    fn name(&self) -> &str {
        "github"
    }

    async fn fetch(&self) -> anyhow::Result<Vec<SkillMeta>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            self.owner, self.repo, self.path
        );

        let config = crate::config::Config::load()?;
        let mut req = self
            .client
            .get(&url)
            .header("User-Agent", "devbase-skill-sync")
            .header("Accept", "application/vnd.github.v3+json");

        if let Some(token) = config.github.token.as_deref() {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "GitHub API returned {} for {}/{}",
                resp.status(),
                self.owner,
                self.repo
            ));
        }

        let entries: Vec<serde_json::Value> = resp.json().await?;
        let mut skills = Vec::new();

        for entry in entries {
            let entry_type = entry["type"].as_str().unwrap_or("");
            let name = entry["name"].as_str().unwrap_or("unknown");

            // Handle directories by recursing (Box::pin required for recursive async fn)
            if entry_type == "dir" {
                let dir_source =
                    GitHubSource::new(&self.owner, &self.repo, &format!("{}/{}", self.path, name));
                let fut = Box::pin(dir_source.fetch());
                if let Ok(dir_skills) = fut.await {
                    skills.extend(dir_skills);
                }
                continue;
            }

            if !name.ends_with(".md") {
                continue;
            }

            let download_url = entry["download_url"].as_str().unwrap_or("");
            let html_url = entry["html_url"].as_str().unwrap_or("");
            if download_url.is_empty() {
                continue;
            }

            let content = match self
                .client
                .get(download_url)
                .header("User-Agent", "devbase-skill-sync")
                .send()
                .await
            {
                Ok(resp) => resp.text().await.unwrap_or_default(),
                Err(_) => continue,
            };

            let skill_id = name.trim_end_matches(".md").to_lowercase().replace('_', "-");

            // Try parsing as SKILL.md first; fall back to plain markdown extraction
            let skill_meta = if content.contains("---") {
                parse_skill_or_extract(&content, &skill_id, html_url)
            } else {
                extract_skill_from_md(&content, &skill_id, html_url)
            };

            skills.push(skill_meta);
        }

        Ok(skills)
    }
}

// ── Local File Source ──────────────────────────────────────────────

pub struct LocalFileSource {
    pub name_str: String,
    pub dir_path: std::path::PathBuf,
}

impl LocalFileSource {
    pub fn new(name: &str, dir_path: &Path) -> Self {
        LocalFileSource {
            name_str: name.to_string(),
            dir_path: dir_path.to_path_buf(),
        }
    }
}

#[async_trait]
impl SkillSource for LocalFileSource {
    fn name(&self) -> &str {
        &self.name_str
    }

    async fn fetch(&self) -> anyhow::Result<Vec<SkillMeta>> {
        let mut skills = Vec::new();
        scan_dir_for_skills(&self.dir_path, &mut skills)?;
        Ok(skills)
    }
}

fn scan_dir_for_skills(dir: &Path, skills: &mut Vec<SkillMeta>) -> anyhow::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir_for_skills(&path, skills)?;
        } else if path.extension().map_or(false, |e| e == "md") {
            let content = std::fs::read_to_string(&path)?;
            let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let skill_id = name.to_lowercase().replace('_', "-");
            let skill_meta = if content.contains("---") {
                parse_skill_or_extract(&content, &skill_id, "")
            } else {
                extract_skill_from_md(&content, &skill_id, "")
            };
            skills.push(skill_meta);
        }
    }
    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────

fn parse_skill_or_extract(content: &str, skill_id: &str, source_url: &str) -> SkillMeta {
    // Try the proper SKILL.md parser first
    let skill_dir = std::env::temp_dir().join(format!("devbase-skill-import-{}", skill_id));
    std::fs::create_dir_all(&skill_dir).ok();
    let skill_md = skill_dir.join("SKILL.md");
    std::fs::write(&skill_md, content).ok();

    let mut skill = crate::skill_runtime::parser::parse_skill_md(&skill_md)
        .unwrap_or_else(|_| extract_skill_from_md(content, skill_id, source_url));

    // Override with our computed id
    let old_id = skill.id.clone();
    skill.id = skill_id.to_string();
    skill.local_path = skill_dir;
    skill.skill_type = SkillType::Custom;

    // If the parser didn't pick up the description, try extracting
    if skill.description.is_empty() || skill.description == old_id {
        skill.description = extract_description(content);
    }

    skill
}

fn extract_skill_from_md(content: &str, skill_id: &str, _source_url: &str) -> SkillMeta {
    let description = extract_description(content);
    let tags = extract_tags(content);

    SkillMeta {
        id: skill_id.to_string(),
        name: skill_id.to_string(),
        version: "0.1.0".to_string(),
        description,
        author: None,
        tags,
        entry_script: None,
        category: None,
        skill_type: SkillType::Custom,
        local_path: std::path::PathBuf::new(),
        inputs: vec![],
        outputs: vec![],
        dependencies: vec![],
        embedding: None,
        installed_at: Utc::now(),
        updated_at: Utc::now(),
        last_used_at: None,
        body: content.to_string(),
    }
}

fn extract_description(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(desc) = trimmed.strip_prefix("description:") {
            return desc.trim().trim_matches('"').to_string();
        }
    }
    content
        .lines()
        .find(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#') && !t.starts_with("---")
        })
        .unwrap_or("")
        .trim()
        .to_string()
}

fn extract_tags(content: &str) -> Vec<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(tags_str) = trimmed.strip_prefix("tags:") {
            return tags_str
                .split(',')
                .map(|t| t.trim().trim_matches('"').trim_matches('[').trim_matches(']').to_string())
                .filter(|t| !t.is_empty())
                .collect();
        }
    }
    vec![]
}
