// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

pub mod field_parsers;
pub mod frontmatter;

use devbase_skill_runtime_types::{SkillInput, SkillMeta, SkillOutput, SkillType};

/// Parse a SKILL.md file into `SkillMeta`.
///
/// The file must contain YAML frontmatter delimited by `---` lines,
/// followed by a Markdown body.
pub fn parse_skill_md(path: &std::path::Path) -> anyhow::Result<SkillMeta> {
    let content = std::fs::read_to_string(path)?;
    let id = SkillMeta::id_from_path(path.parent().unwrap_or(path));

    let (frontmatter, body) = if let Some((fm, offset)) = frontmatter::extract_frontmatter(&content)
    {
        (fm, content[offset..].trim_start().to_string())
    } else {
        // No frontmatter: treat entire file as body with minimal defaults
        let id = id.clone();
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or(&id).to_string();
        return Ok(SkillMeta {
            id: id.clone(),
            name: name.clone(),
            version: "0.1.0".to_string(),
            description: name,
            author: None,
            tags: Vec::new(),
            entry_script: None,
            skill_type: SkillType::Custom,
            local_path: path.parent().unwrap_or(path).to_path_buf(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            dependencies: Vec::new(),
            embedding: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_used_at: None,
            body: content,
            category: None,
        });
    };

    let inputs = frontmatter
        .inputs
        .iter()
        .map(|inp| SkillInput {
            name: inp.name.clone(),
            input_type: inp.input_type.clone(),
            description: inp.description.clone(),
            required: inp.required,
            default: inp.default.clone(),
        })
        .collect();

    let outputs = frontmatter
        .outputs
        .iter()
        .map(|out| SkillOutput {
            name: out.name.clone(),
            output_type: out.output_type.clone(),
            description: out.description.clone(),
        })
        .collect();

    let dependencies = frontmatter.dependencies.clone();

    let now = chrono::Utc::now();
    Ok(SkillMeta {
        id: frontmatter.id.clone().unwrap_or_else(|| id.clone()),
        name: frontmatter.name.clone().unwrap_or_else(|| id.clone()),
        version: frontmatter.version.clone().unwrap_or_else(|| "0.1.0".to_string()),
        description: frontmatter.description.clone().unwrap_or_default(),
        author: frontmatter.author.clone(),
        tags: frontmatter.tags.clone(),
        entry_script: frontmatter.entry_script.clone(),
        skill_type: frontmatter
            .skill_type
            .as_deref()
            .map(|s| s.parse().unwrap_or(SkillType::Custom))
            .unwrap_or(SkillType::Custom),
        local_path: path.parent().unwrap_or(path).to_path_buf(),
        inputs,
        outputs,
        dependencies,
        embedding: None,
        installed_at: now,
        updated_at: now,
        last_used_at: None,
        body,
        category: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_skill_md() {
        let md = r#"---
name: code-audit
version: "1.0.0"
description: Audit a Rust codebase for common issues
author: devbase-team
tags: [rust, audit, security]
inputs:
  - name: repo_id
    type: string
    description: Target repository ID
    required: true
  - name: severity
    type: string
    description: Minimum severity
    default: "warning"
outputs:
  - name: report
    type: markdown
    description: Audit report
---
# Code Audit Skill

This skill audits a Rust codebase...
"#;
        let dir = std::env::temp_dir().join("test-skill");
        let path = dir.join("SKILL.md");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, md).unwrap();

        let skill = parse_skill_md(&path).unwrap();
        assert_eq!(skill.id, "test-skill");
        assert_eq!(skill.name, "code-audit");
        assert_eq!(skill.version, "1.0.0");
        assert_eq!(skill.tags, vec!["rust", "audit", "security"]);
        assert_eq!(skill.inputs.len(), 2);
        assert_eq!(skill.inputs[0].name, "repo_id");
        assert!(skill.inputs[0].required);
        assert_eq!(skill.inputs[1].default, Some("warning".to_string()));
        assert_eq!(skill.outputs.len(), 1);
        assert_eq!(skill.outputs[0].name, "report");
        assert!(skill.body.contains("Code Audit Skill"));

        // cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let dir = std::env::temp_dir().join("test-skill-raw");
        let path = dir.join("SKILL.md");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "# Raw Skill\n\nNo frontmatter.").unwrap();

        let skill = parse_skill_md(&path).unwrap();
        assert_eq!(skill.id, "test-skill-raw");
        assert_eq!(skill.name, "SKILL");
        assert_eq!(skill.version, "0.1.0");
        assert!(skill.body.contains("No frontmatter"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_real_embed_repo() {
        let path = std::path::PathBuf::from("skills/embed-repo/SKILL.md");
        if !path.exists() {
            return; // skip if not in right cwd
        }
        let skill = parse_skill_md(&path).unwrap();
        assert_eq!(skill.name, "embed-repo");
        assert_eq!(skill.version, "1.0.0");
        assert_eq!(skill.skill_type, SkillType::Builtin);
        assert_eq!(
            skill.description,
            "Generate semantic embeddings for a repository's code symbols"
        );
        assert_eq!(skill.tags, vec!["embedding", "semantic-search", "indexing"]);
        assert_eq!(skill.inputs.len(), 2);
        assert_eq!(skill.outputs.len(), 1);
        assert_eq!(skill.outputs[0].name, "status");
    }
}
