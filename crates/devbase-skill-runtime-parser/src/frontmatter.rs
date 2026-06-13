// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use devbase_skill_runtime_types::{SkillDependency, SkillInput, SkillOutput};

/// Parsed frontmatter specific to SKILL.md.
#[derive(Debug, Clone, Default)]
pub struct SkillFrontmatter {
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
pub fn extract_frontmatter(content: &str) -> Option<(SkillFrontmatter, usize)> {
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

pub fn parse_skill_frontmatter(raw: &str) -> SkillFrontmatter {
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
        if let Some(item) = trimmed.strip_prefix("- ") {
            match current_section {
                Some("inputs") => {
                    // Flush previous input if we see a new "- name:" without closing the last one
                    if item.starts_with("name:")
                        && let Some(input) = current_input.take()
                    {
                        fm.inputs.push(input);
                    }
                    if current_input.is_none() && item.starts_with("name:") {
                        current_input = Some(SkillInput::default());
                    }
                    if let Some(ref mut inp) = current_input {
                        super::field_parsers::parse_input_field(item, inp);
                    }
                }
                Some("outputs") => {
                    if item.starts_with("name:")
                        && let Some(output) = current_output.take()
                    {
                        fm.outputs.push(output);
                    }
                    if current_output.is_none() && item.starts_with("name:") {
                        current_output = Some(SkillOutput::default());
                    }
                    if let Some(ref mut out) = current_output {
                        super::field_parsers::parse_output_field(item, out);
                    }
                }
                Some("dependencies") => {
                    if item.starts_with("id:") {
                        fm.dependencies.push(super::field_parsers::parse_dependency_item(item));
                    } else if let Some(last) = fm.dependencies.last_mut() {
                        super::field_parsers::parse_dependency_field(item, last);
                    }
                }
                _ => {
                    // Top-level list (e.g. tags inline)
                    if let Some((key, _)) = line.split_once(':') {
                        let key = key.trim();
                        if key == "tags" {
                            fm.tags.push(super::field_parsers::unquote(item).to_string());
                        }
                    }
                }
            }
            continue;
        }

        // Flush any open input/output before moving to a new key
        if trimmed.starts_with("name:") {
            if let Some(input) = current_input.take() {
                fm.inputs.push(input);
            }
            current_input = Some(SkillInput::default());
        }
        if trimmed.starts_with("name:") {
            if let Some(output) = current_output.take() {
                fm.outputs.push(output);
            }
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
                    super::field_parsers::parse_input_field(trimmed, inp);
                    continue;
                }
                if let Some(ref mut out) = current_output {
                    super::field_parsers::parse_output_field(trimmed, out);
                    continue;
                }
            }

            // Top-level fields
            match key {
                "id" => fm.id = Some(super::field_parsers::unquote(rest).to_string()),
                "name" => fm.name = Some(super::field_parsers::unquote(rest).to_string()),
                "version" => fm.version = Some(super::field_parsers::unquote(rest).to_string()),
                "description" => {
                    fm.description = Some(super::field_parsers::unquote(rest).to_string())
                }
                "author" => fm.author = Some(super::field_parsers::unquote(rest).to_string()),
                "entry_script" => {
                    fm.entry_script = Some(super::field_parsers::unquote(rest).to_string())
                }
                "skill_type" | "type" => {
                    fm.skill_type = Some(super::field_parsers::unquote(rest).to_string())
                }
                "tags" => {
                    current_section = None;
                    if rest.starts_with('[') && rest.ends_with(']') {
                        fm.tags = rest[1..rest.len() - 1]
                            .split(',')
                            .map(|s| super::field_parsers::unquote(s.trim()).to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    } else if !rest.is_empty() {
                        fm.tags = vec![super::field_parsers::unquote(rest).to_string()];
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
