// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

pub mod execution;
pub mod params;
pub mod skill_type;

// Re-export for backward compatibility
pub use execution::{ExecutionResult, ExecutionStatus};
pub use params::{SkillDependency, SkillInput, SkillOutput};
pub use skill_type::SkillType;

use chrono::{DateTime, Utc};

/// In-memory representation of a parsed SKILL.md + registry metadata.
#[derive(Debug, Clone)]
pub struct SkillMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub entry_script: Option<String>,
    pub skill_type: SkillType,
    pub local_path: std::path::PathBuf,
    pub inputs: Vec<SkillInput>,
    pub outputs: Vec<SkillOutput>,
    pub dependencies: Vec<SkillDependency>,
    pub embedding: Option<Vec<f32>>,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    /// Markdown body after the YAML frontmatter.
    pub body: String,
    /// Taxonomy category (ai, dev, data, infra, communication + sub-category).
    pub category: Option<String>,
}

impl SkillMeta {
    /// Derive the skill ID from its directory name (kebab-case).
    pub fn id_from_path(path: &std::path::Path) -> String {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown-skill")
            .to_lowercase()
            .replace([' ', '_'], "-")
    }

    /// Default entry script path relative to the skill directory.
    pub fn default_entry_script(&self) -> Option<String> {
        self.local_path
            .join("scripts")
            .join("run.py")
            .exists()
            .then_some("scripts/run.py".to_string())
    }
}

/// Lightweight row from the `skills` table (without body/embedding blob).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillRow {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub entry_script: Option<String>,
    pub skill_type: SkillType,
    pub local_path: String,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub dependencies: Vec<SkillDependency>,
    pub category: Option<String>,
}

/// Helper: parse a JSON tags array or fall back to CSV.
pub fn parse_tags(tags_str: Option<&str>) -> Vec<String> {
    let Some(s) = tags_str else {
        return Vec::new();
    };
    if s.trim().starts_with('[') {
        serde_json::from_str(s).unwrap_or_default()
    } else {
        s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
    }
}

/// Helper: serialize tags to JSON array string.
pub fn serialize_tags(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_type_roundtrip() {
        assert_eq!(SkillType::Builtin.as_str(), "builtin");
        assert_eq!(SkillType::Custom.as_str(), "custom");
        assert_eq!(SkillType::System.as_str(), "system");
        assert_eq!("builtin".parse::<SkillType>().unwrap(), SkillType::Builtin);
        assert_eq!("custom".parse::<SkillType>().unwrap(), SkillType::Custom);
        assert_eq!("system".parse::<SkillType>().unwrap(), SkillType::System);
    }

    #[test]
    fn test_execution_status_roundtrip() {
        assert_eq!(ExecutionStatus::Pending.as_str(), "pending");
        assert_eq!(ExecutionStatus::Success.as_str(), "success");
        assert_eq!(ExecutionStatus::Timeout.as_str(), "timeout");
        assert_eq!("running".parse::<ExecutionStatus>().unwrap(), ExecutionStatus::Running);
        assert_eq!("failed".parse::<ExecutionStatus>().unwrap(), ExecutionStatus::Failed);
    }

    #[test]
    fn test_skill_meta_id_from_path() {
        assert_eq!(SkillMeta::id_from_path(std::path::Path::new("/skills/My Skill")), "my-skill");
        assert_eq!(SkillMeta::id_from_path(std::path::Path::new("/skills/my_skill")), "my-skill");
        assert_eq!(SkillMeta::id_from_path(std::path::Path::new("/skills/my-skill")), "my-skill");
    }

    #[test]
    fn test_parse_tags_csv() {
        assert_eq!(parse_tags(Some("rust, cli,  ai")), vec!["rust", "cli", "ai"]);
    }

    #[test]
    fn test_parse_tags_json() {
        assert_eq!(parse_tags(Some("[\"rust\", \"cli\", \"ai\"]")), vec!["rust", "cli", "ai"]);
    }

    #[test]
    fn test_parse_tags_empty() {
        assert!(parse_tags(None).is_empty());
        assert!(parse_tags(Some("")).is_empty());
    }

    #[test]
    fn test_serialize_tags_roundtrip() {
        let tags = vec!["rust".to_string(), "cli".to_string()];
        let json = serialize_tags(&tags);
        assert_eq!(parse_tags(Some(&json)), tags);
    }
}
