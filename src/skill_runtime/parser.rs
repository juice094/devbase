use super::{SkillDependency, SkillInput, SkillMeta, SkillOutput, SkillType};

/// Parse a SKILL.md file into `SkillMeta`.
///
/// The file must contain YAML frontmatter delimited by `---` lines,
/// followed by a Markdown body.
pub fn parse_skill_md(path: &std::path::Path) -> anyhow::Result<SkillMeta> {
    let content = std::fs::read_to_string(path)?;
    let id = SkillMeta::id_from_path(path.parent().unwrap_or(path));

    let (frontmatter, body) = if let Some((fm, offset)) = extract_frontmatter(&content) {
        (fm, content[offset..].trim_start().to_string())
    } else {
        // No frontmatter: treat entire file as body with minimal defaults
        let id = id.clone();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&id)
            .to_string();
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

/// Parsed frontmatter specific to SKILL.md.
#[derive(Debug, Clone, Default)]
struct SkillFrontmatter {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub entry_script: Option<String>,
    pub skill_type: Option<String>,
    pub inputs: Vec<SkillInput>,
    pub outputs: Vec<SkillOutput>,
    pub dependencies: Vec<SkillDependency>,
}

/// Extract YAML frontmatter from the top of a Markdown document.
fn extract_frontmatter(content: &str) -> Option<(SkillFrontmatter, usize)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_open = &trimmed[3..];
    let close_pos = after_open.find("\n---")?;
    let raw = after_open[..close_pos].trim();
    let body_offset = trimmed.as_ptr() as usize - content.as_ptr() as usize + 3 + close_pos + 4;

    let fm = parse_skill_frontmatter(raw);
    Some((fm, body_offset))
}

fn parse_skill_frontmatter(raw: &str) -> SkillFrontmatter {
    let mut fm = SkillFrontmatter::default();
    let mut current_section: Option<&str> = None;
    let mut current_input: Option<SkillInput> = None;
    let mut current_output: Option<SkillOutput> = None;

    for line in raw.lines() {
        let line = line.trim_end();
        let trimmed = line.trim_start();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // YAML list item within a section
        if trimmed.starts_with("- ") {
            let item = &trimmed[2..];
            match current_section {
                Some("inputs") => {
                    // Flush previous input if we see a new "- name:" without closing the last one
                    if item.starts_with("name:") && current_input.is_some() {
                        fm.inputs.push(current_input.take().unwrap());
                    }
                    if current_input.is_none() && item.starts_with("name:") {
                        current_input = Some(SkillInput::default());
                    }
                    if let Some(ref mut inp) = current_input {
                        parse_input_field(item, inp);
                    }
                }
                Some("outputs") => {
                    if item.starts_with("name:") && current_output.is_some() {
                        fm.outputs.push(current_output.take().unwrap());
                    }
                    if current_output.is_none() && item.starts_with("name:") {
                        current_output = Some(SkillOutput::default());
                    }
                    if let Some(ref mut out) = current_output {
                        parse_output_field(item, out);
                    }
                }
                Some("dependencies") => {
                    if item.starts_with("id:") {
                        fm.dependencies.push(parse_dependency_item(item));
                    } else if let Some(last) = fm.dependencies.last_mut() {
                        parse_dependency_field(item, last);
                    }
                }
                _ => {
                    // Top-level list (e.g. tags inline)
                    if let Some((key, _)) = line.split_once(':') {
                        let key = key.trim();
                        if key == "tags" {
                            fm.tags.push(unquote(item).to_string());
                        }
                    }
                }
            }
            continue;
        }

        // Flush any open input/output before moving to a new key
        if current_input.is_some() && trimmed.starts_with("name:") {
            fm.inputs.push(current_input.take().unwrap());
            current_input = Some(SkillInput::default());
        }
        if current_output.is_some() && trimmed.starts_with("name:") {
            fm.outputs.push(current_output.take().unwrap());
            current_output = Some(SkillOutput::default());
        }

        if let Some((key, rest)) = trimmed.split_once(':') {
            let key = key.trim();
            let rest = rest.trim();

            // Section starters: inputs / outputs
            if key == "inputs" {
                current_section = Some("inputs");
                continue;
            }
            if key == "outputs" {
                current_section = Some("outputs");
                continue;
            }
            if key == "dependencies" {
                current_section = Some("dependencies");
                continue;
            }

            // If we're inside an input/output block and this is NOT a top-level key,
            // treat it as a nested field.
            // CRITICAL: when current_input / current_output is active, ALL non-section
            // keys must be treated as nested fields, even if they share a name with a
            // top-level field (e.g. "type", "description").
            let is_section_starter = key == "inputs" || key == "outputs";

            if !is_section_starter {
                if let Some(ref mut inp) = current_input {
                    parse_input_field(trimmed, inp);
                    continue;
                }
                if let Some(ref mut out) = current_output {
                    parse_output_field(trimmed, out);
                    continue;
                }
            }

            // Top-level fields
            match key {
                "id" => fm.id = Some(unquote(rest).to_string()),
                "name" => fm.name = Some(unquote(rest).to_string()),
                "version" => fm.version = Some(unquote(rest).to_string()),
                "description" => fm.description = Some(unquote(rest).to_string()),
                "author" => fm.author = Some(unquote(rest).to_string()),
                "entry_script" => fm.entry_script = Some(unquote(rest).to_string()),
                "skill_type" | "type" => fm.skill_type = Some(unquote(rest).to_string()),
                "tags" => {
                    current_section = None;
                    if rest.starts_with('[') && rest.ends_with(']') {
                        fm.tags = rest[1..rest.len() - 1]
                            .split(',')
                            .map(|s| unquote(s.trim()).to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    } else if !rest.is_empty() {
                        fm.tags = vec![unquote(rest).to_string()];
                    } else {
                        current_section = Some("tags");
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(inp) = current_input {
        fm.inputs.push(inp);
    }
    if let Some(out) = current_output {
        fm.outputs.push(out);
    }

    fm
}

fn parse_input_field(line: &str, inp: &mut SkillInput) {
    if let Some((key, rest)) = line.split_once(':') {
        let key = key.trim();
        let rest = rest.trim();
        match key {
            "name" => inp.name = unquote(rest).to_string(),
            "type" => inp.input_type = unquote(rest).to_string(),
            "description" => inp.description = unquote(rest).to_string(),
            "required" => inp.required = parse_bool(rest),
            "default" => inp.default = Some(unquote(rest).to_string()),
            _ => {}
        }
    }
}

fn parse_output_field(line: &str, out: &mut SkillOutput) {
    if let Some((key, rest)) = line.split_once(':') {
        let key = key.trim();
        let rest = rest.trim();
        match key {
            "name" => out.name = unquote(rest).to_string(),
            "type" => out.output_type = unquote(rest).to_string(),
            "description" => out.description = unquote(rest).to_string(),
            _ => {}
        }
    }
}

fn parse_dependency_item(item: &str) -> SkillDependency {
    let mut dep = SkillDependency::default();
    if let Some((_, rest)) = item.split_once(':') {
        dep.id = unquote(rest.trim()).to_string();
    }
    dep
}

fn parse_dependency_field(line: &str, dep: &mut SkillDependency) {
    if let Some((key, rest)) = line.split_once(':') {
        let key = key.trim();
        let rest = rest.trim();
        match key {
            "version" => dep.version = Some(unquote(rest).to_string()),
            "source" => dep.source = Some(unquote(rest).to_string()),
            _ => {}
        }
    }
}

fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_lowercase().as_str(), "true" | "yes" | "1" | "on")
}

fn unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(s)
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
        assert_eq!(skill.skill_type, super::SkillType::Builtin);
        assert_eq!(skill.description, "Generate semantic embeddings for a repository's code symbols");
        assert_eq!(skill.tags, vec!["embedding", "semantic-search", "indexing"]);
        assert_eq!(skill.inputs.len(), 2);
        assert_eq!(skill.outputs.len(), 1);
        assert_eq!(skill.outputs[0].name, "status");
    }
}
