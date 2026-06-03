// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

/// Skill type discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillType {
    /// Distributed with devbase; always available.
    Builtin,
    /// Installed by user from external source.
    Custom,
    /// Reserved for devbase-internal system utilities.
    System,
}

impl SkillType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillType::Builtin => "builtin",
            SkillType::Custom => "custom",
            SkillType::System => "system",
        }
    }
}

impl std::str::FromStr for SkillType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "builtin" => Ok(SkillType::Builtin),
            "custom" => Ok(SkillType::Custom),
            "system" => Ok(SkillType::System),
            _ => Err(anyhow::anyhow!("unknown skill_type: {}", s)),
        }
    }
}
