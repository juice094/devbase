// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use devbase_skill_runtime_types::{SkillDependency, SkillInput, SkillOutput};

pub fn parse_input_field(line: &str, inp: &mut SkillInput) {
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

pub fn parse_output_field(line: &str, out: &mut SkillOutput) {
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

pub fn parse_dependency_item(item: &str) -> SkillDependency {
    let mut dep = SkillDependency::default();
    if let Some((_, rest)) = item.split_once(':') {
        dep.id = unquote(rest.trim()).to_string();
    }
    dep
}

pub fn parse_dependency_field(line: &str, dep: &mut SkillDependency) {
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

pub fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_lowercase().as_str(), "true" | "yes" | "1" | "on")
}

pub fn unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(s)
}
