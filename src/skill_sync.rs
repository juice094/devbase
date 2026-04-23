//! Sync vault notes with ai_context=true to Clarity SKILL.md format.

use crate::vault::frontmatter::extract_frontmatter;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Scan the vault for AI-relevant notes and export them as Clarity SKILL.md files.
///
/// * `output_dir` — destination directory for generated SKILL.md files.
/// * `filter_tags` — only sync notes that have at least one of these tags.
/// * `dry_run` — if true, only print what would be synced.
///
/// Returns the number of notes synced.
pub fn run_sync(output_dir: &str, filter_tags: &[String], dry_run: bool) -> anyhow::Result<usize> {
    let vault_dir = crate::registry::WorkspaceRegistry::workspace_dir()?.join("vault");
    let output = PathBuf::from(output_dir);

    if !dry_run {
        fs::create_dir_all(&output)?;
    }

    let filter_set: HashSet<String> = filter_tags.iter().cloned().collect();
    let mut synced = 0;

    for entry in walkdir::WalkDir::new(&vault_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().map(|ext| ext == "md").unwrap_or(false)
        })
    {
        let path = entry.path();
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Skipping {}: {}", path.display(), e);
                continue;
            }
        };

        let (frontmatter, body_offset) = match extract_frontmatter(&content) {
            Some((fm, off)) => (fm, off),
            None => continue,
        };

        // Only sync notes explicitly marked for AI context
        if !frontmatter.ai_context.unwrap_or(false) {
            continue;
        }

        // Tag filtering
        if !filter_set.is_empty() {
            let note_tags: HashSet<String> = frontmatter.tags.iter().cloned().collect();
            if !filter_set.iter().any(|t| note_tags.contains(t)) {
                continue;
            }
        }

        let body = &content[body_offset..];
        let skill_content = convert_to_skill(&frontmatter, body);

        let id = frontmatter
            .id
            .clone()
            .unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().to_string());
        let out_path = output.join(format!("{}.md", id));

        if dry_run {
            println!("[dry-run] Would write: {}", out_path.display());
        } else {
            fs::write(&out_path, skill_content)?;
        }

        synced += 1;
    }

    Ok(synced)
}

/// Convert a vault note into Clarity SKILL.md format.
fn convert_to_skill(fm: &crate::vault::frontmatter::Frontmatter, body: &str) -> String {
    let id = fm.id.as_deref().unwrap_or("unnamed-skill");
    let name = fm.title.as_deref().unwrap_or(id);
    let description = extract_description(body);

    let tags_yaml = if fm.tags.is_empty() {
        String::new()
    } else {
        fm.tags.iter().map(|t| format!("- {}", t)).collect::<Vec<_>>().join("\n")
    };

    format!(
        r#"---
id: {}
name: {}
version: "1.0.0"
description: {}
tags:
{}
---

{}"#,
        id,
        name,
        description,
        tags_yaml,
        body.trim()
    )
}

/// Extract a one-line description from the note body (first non-empty, non-heading paragraph).
fn extract_description(body: &str) -> String {
    body.lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            if l.len() > 120 {
                format!("{}...", &l[..117])
            } else {
                l.to_string()
            }
        })
        .unwrap_or_else(|| "Vault note synced from devbase".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::frontmatter::Frontmatter;

    #[test]
    fn test_extract_description_short() {
        let body = "\n\nThis is a short description.\n\nMore content...";
        assert_eq!(extract_description(body), "This is a short description.");
    }

    #[test]
    fn test_extract_description_long() {
        let body = "a".repeat(200);
        let desc = extract_description(&body);
        assert!(desc.len() <= 120);
        assert!(desc.ends_with("..."));
    }

    #[test]
    fn test_extract_description_skips_heading() {
        let body = "# Title\n\nActual description here.";
        assert_eq!(extract_description(body), "Actual description here.");
    }

    #[test]
    fn test_convert_to_skill() {
        let fm = Frontmatter {
            id: Some("test-skill".to_string()),
            title: Some("Test Skill".to_string()),
            tags: vec!["rust".to_string(), "deploy".to_string()],
            ..Default::default()
        };
        let body = "## Step 1\nDo something.";
        let skill = convert_to_skill(&fm, body);
        assert!(skill.contains("id: test-skill"));
        assert!(skill.contains("name: Test Skill"));
        assert!(skill.contains("- rust"));
        assert!(skill.contains("- deploy"));
        assert!(skill.contains("## Step 1"));
    }
}
