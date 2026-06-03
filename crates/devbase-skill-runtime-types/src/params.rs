// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

/// A single input parameter declared in SKILL.md.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SkillInput {
    pub name: String,
    pub input_type: String,
    pub description: String,
    pub required: bool,
    pub default: Option<String>,
}

/// A single output parameter declared in SKILL.md.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SkillOutput {
    pub name: String,
    pub output_type: String,
    pub description: String,
}

/// A dependency declared by a skill on another skill.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SkillDependency {
    pub id: String,
    pub version: Option<String>,
    pub source: Option<String>,
}
