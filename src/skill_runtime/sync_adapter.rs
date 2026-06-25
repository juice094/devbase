// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094
//! Adapters for syncing devbase skills to multiple client-specific formats.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};

use crate::skill_runtime::SkillMeta;

/// Sync a list of devbase skills to a client-specific output directory.
pub trait SkillSyncAdapter {
    fn name(&self) -> &'static str;
    fn sync(&self, skills: &[SkillMeta], output_dir: &Path) -> Result<usize>;
}

/// Resolve an adapter by its short name.
pub fn resolve_adapter(name: &str) -> Option<Box<dyn SkillSyncAdapter>> {
    match name.to_lowercase().as_str() {
        "clarity" | "clarity-json" | "json" => Some(Box::new(ClarityJsonAdapter)),
        "kimi" | "kimicli" => Some(Box::new(KimiCliAdapter)),
        "claude" | "claude-code" | "claudecode" => Some(Box::new(ClaudeCodeAdapter)),
        "codex" => Some(Box::new(CodexAdapter)),
        "claw" => Some(Box::new(ClawAdapter)),
        _ => None,
    }
}

/// List all supported adapter short names.
pub fn supported_adapters() -> Vec<&'static str> {
    vec!["clarity", "kimicli", "claude-code", "codex", "claw"]
}

/// Resolve multiple adapter names. Returns an error if any name is unknown.
pub fn resolve_adapters(names: &[String]) -> Result<Vec<Box<dyn SkillSyncAdapter>>> {
    let mut adapters: Vec<Box<dyn SkillSyncAdapter>> = Vec::new();
    let mut seen = HashSet::new();
    for name in names {
        let adapter = resolve_adapter(name)
            .with_context(|| format!("Unknown skill sync target: '{}'", name))?;
        if seen.insert(adapter.name()) {
            adapters.push(adapter);
        }
    }
    Ok(adapters)
}

// ---------------------------------------------------------------------------
// Clarity JSON adapter (legacy / default)
// ---------------------------------------------------------------------------

pub struct ClarityJsonAdapter;

impl SkillSyncAdapter for ClarityJsonAdapter {
    fn name(&self) -> &'static str {
        "clarity"
    }

    fn sync(&self, skills: &[SkillMeta], output_dir: &Path) -> Result<usize> {
        crate::skill_runtime::clarity_sync::sync_skills_to_plans_with_skills(output_dir, skills)
    }
}

// ---------------------------------------------------------------------------
// Markdown skill adapter base (Kimi CLI / Claude Code / Codex / claw)
// ---------------------------------------------------------------------------

/// Output layout for a Markdown-based skill adapter.
pub struct MarkdownLayout {
    /// Root output directory.
    pub output_dir: &'static str,
    /// Whether to nest skills under `.client/skills/<id>/SKILL.md`.
    pub nested_under_dotdir: bool,
    /// Optional extra frontmatter fields rendered as a raw YAML string.
    pub extra_frontmatter: Option<&'static str>,
}

pub struct MarkdownSkillAdapter {
    pub name: &'static str,
    pub layout: MarkdownLayout,
}

impl MarkdownSkillAdapter {
    fn write_skill(&self, skill: &SkillMeta, output_dir: &Path) -> Result<()> {
        let skill_dir = if self.layout.nested_under_dotdir {
            output_dir.join(self.layout.output_dir).join("skills").join(&skill.id)
        } else {
            output_dir.join(self.layout.output_dir).join(&skill.id)
        };
        std::fs::create_dir_all(&skill_dir)
            .with_context(|| format!("Failed to create skill dir: {}", skill_dir.display()))?;

        let skill_md = skill_dir.join("SKILL.md");
        let content = render_skill_md(skill, self.layout.extra_frontmatter);
        std::fs::write(&skill_md, content)
            .with_context(|| format!("Failed to write skill: {}", skill_md.display()))?;
        Ok(())
    }
}

impl SkillSyncAdapter for MarkdownSkillAdapter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn sync(&self, skills: &[SkillMeta], output_dir: &Path) -> Result<usize> {
        std::fs::create_dir_all(output_dir)
            .with_context(|| format!("Failed to create output dir: {}", output_dir.display()))?;
        for skill in skills {
            self.write_skill(skill, output_dir)?;
        }
        Ok(skills.len())
    }
}

fn render_skill_md(skill: &SkillMeta, extra_frontmatter: Option<&str>) -> String {
    let mut fm = String::new();
    fm.push_str(&format!("name: {}\n", skill.name));
    fm.push_str(&format!(
        "description: {}\n",
        serde_yaml::to_string(&skill.description)
            .unwrap_or_default()
            .trim_start_matches("---\n")
            .trim()
    ));
    fm.push_str(&format!("version: {}\n", skill.version));
    if !skill.tags.is_empty() {
        fm.push_str(&format!(
            "tags: {}\n",
            serde_yaml::to_string(&skill.tags)
                .unwrap_or_default()
                .trim_start_matches("---\n")
                .trim()
        ));
    }
    if let Some(author) = &skill.author {
        fm.push_str(&format!("author: {}\n", author));
    }
    if let Some(category) = &skill.category {
        fm.push_str(&format!("category: {}\n", category));
    }
    if let Some(extra) = extra_frontmatter {
        fm.push_str(extra);
    }

    let mut output = String::new();
    output.push_str("---\n");
    output.push_str(&fm);
    output.push_str("---\n\n");
    output.push_str(&skill.body);
    output.push('\n');
    output
}

// ---------------------------------------------------------------------------
// Client-specific Markdown adapters
// ---------------------------------------------------------------------------

pub struct KimiCliAdapter;

impl SkillSyncAdapter for KimiCliAdapter {
    fn name(&self) -> &'static str {
        "kimicli"
    }

    fn sync(&self, skills: &[SkillMeta], output_dir: &Path) -> Result<usize> {
        MarkdownSkillAdapter {
            name: "kimicli",
            layout: MarkdownLayout {
                output_dir: "",
                nested_under_dotdir: false,
                extra_frontmatter: None,
            },
        }
        .sync(skills, output_dir)
    }
}

pub struct ClaudeCodeAdapter;

impl SkillSyncAdapter for ClaudeCodeAdapter {
    fn name(&self) -> &'static str {
        "claude-code"
    }

    fn sync(&self, skills: &[SkillMeta], output_dir: &Path) -> Result<usize> {
        MarkdownSkillAdapter {
            name: "claude-code",
            layout: MarkdownLayout {
                output_dir: ".claude",
                nested_under_dotdir: true,
                extra_frontmatter: None,
            },
        }
        .sync(skills, output_dir)
    }
}

pub struct CodexAdapter;

impl SkillSyncAdapter for CodexAdapter {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn sync(&self, skills: &[SkillMeta], output_dir: &Path) -> Result<usize> {
        MarkdownSkillAdapter {
            name: "codex",
            layout: MarkdownLayout {
                output_dir: ".codex",
                nested_under_dotdir: true,
                extra_frontmatter: None,
            },
        }
        .sync(skills, output_dir)
    }
}

pub struct ClawAdapter;

impl SkillSyncAdapter for ClawAdapter {
    fn name(&self) -> &'static str {
        "claw"
    }

    fn sync(&self, skills: &[SkillMeta], output_dir: &Path) -> Result<usize> {
        // claw currently consumes Kimi CLI format from .kimi/skills/
        MarkdownSkillAdapter {
            name: "claw",
            layout: MarkdownLayout {
                output_dir: ".kimi",
                nested_under_dotdir: true,
                extra_frontmatter: None,
            },
        }
        .sync(skills, output_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use devbase_skill_runtime_types::{SkillMeta, SkillType};

    fn dummy_skill(id: &str, body: &str) -> SkillMeta {
        SkillMeta {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            description: format!("Description for {}", id),
            author: Some("devbase".to_string()),
            tags: vec!["rust".to_string()],
            entry_script: None,
            skill_type: SkillType::Custom,
            local_path: std::path::PathBuf::from(format!("/tmp/{}", id)),
            inputs: vec![],
            outputs: vec![],
            dependencies: vec![],
            embedding: None,
            installed_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
            body: body.to_string(),
            category: Some("dev".to_string()),
        }
    }

    #[test]
    fn test_resolve_adapter_known() {
        assert!(resolve_adapter("kimicli").is_some());
        assert!(resolve_adapter("claude-code").is_some());
        assert!(resolve_adapter("codex").is_some());
        assert!(resolve_adapter("claw").is_some());
        assert!(resolve_adapter("clarity").is_some());
    }

    #[test]
    fn test_resolve_adapter_unknown() {
        assert!(resolve_adapter("vscode").is_none());
    }

    #[test]
    fn test_kimi_adapter_writes_skill_md() {
        let tmp = tempfile::tempdir().unwrap();
        let skill = dummy_skill("test-skill", "# Test\n\nBody.");
        let adapter = KimiCliAdapter;
        let count = adapter.sync(&[skill], tmp.path()).unwrap();
        assert_eq!(count, 1);
        let path = tmp.path().join("test-skill").join("SKILL.md");
        assert!(path.exists());
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("name: test-skill"));
        assert!(content.contains("# Test"));
    }

    #[test]
    fn test_claude_adapter_writes_under_dotdir() {
        let tmp = tempfile::tempdir().unwrap();
        let skill = dummy_skill("claude-skill", "Body");
        let adapter = ClaudeCodeAdapter;
        adapter.sync(&[skill], tmp.path()).unwrap();
        let path = tmp.path().join(".claude").join("skills").join("claude-skill").join("SKILL.md");
        assert!(path.exists());
    }

    #[test]
    fn test_claw_adapter_writes_under_kimi_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let skill = dummy_skill("claw-skill", "Body");
        let adapter = ClawAdapter;
        adapter.sync(&[skill], tmp.path()).unwrap();
        let path = tmp.path().join(".kimi").join("skills").join("claw-skill").join("SKILL.md");
        assert!(path.exists());
    }
}
