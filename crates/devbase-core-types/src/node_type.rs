// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094

/// Types of knowledge assets managed by devbase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    /// A Git repository (original devbase focus).
    GitRepo,
    /// A Markdown note in the vault (Obsidian-compatible).
    VaultNote,
    /// A binary asset (PDF, image, design file).
    Asset,
    /// An external link (Figma, Notion, API doc).
    ExternalLink,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::GitRepo => write!(f, "repo"),
            NodeType::VaultNote => write!(f, "vault"),
            NodeType::Asset => write!(f, "asset"),
            NodeType::ExternalLink => write!(f, "link"),
        }
    }
}

impl std::str::FromStr for NodeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "repo" | "git" => Ok(NodeType::GitRepo),
            "vault" | "note" => Ok(NodeType::VaultNote),
            "asset" => Ok(NodeType::Asset),
            "link" | "external" => Ok(NodeType::ExternalLink),
            _ => Err(format!("unknown node type: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type_display() {
        assert_eq!(NodeType::GitRepo.to_string(), "repo");
        assert_eq!(NodeType::VaultNote.to_string(), "vault");
    }

    #[test]
    fn test_node_type_from_str() {
        assert_eq!("repo".parse::<NodeType>().unwrap(), NodeType::GitRepo);
        assert_eq!("vault".parse::<NodeType>().unwrap(), NodeType::VaultNote);
        assert!("unknown".parse::<NodeType>().is_err());
    }
}
