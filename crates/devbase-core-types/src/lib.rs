//! devbase-core-types — Core knowledge graph types: Node, Edge, NodeType.
//!
//! **提取日期**: 2026-05-01 (Workspace split)
//! **零内部耦合**: 此 crate 不依赖 devbase 任何内部模块，仅使用 chrono + std。
//! **职责**: 定义 devbase 统一实体模型的基础类型，替代早期的 repo-centric `RepoEntry` 垄断。
//! **边界**: 纯数据结构，无业务逻辑。所有字段 `pub`，无方法（除 `fm()` 便捷访问器）。
//!
//! 与 devbase 的关系: 被 `registry/`、`vault/`、`search/`、`mcp/tools` 等几乎所有模块引用。
//! 作为 workspace 中最底层的类型 crate，禁止引入任何上层依赖。
//!
//! Design decisions:
//! - NodeType 支持 FromStr: 允许 CLI/MCP 通过字符串灵活指定类型（"repo" / "git" / "vault" / "note"）。
//! - frontmatter 用 HashMap<String, String>: 简单、通用，不绑定 YAML 解析器。
//! - Edge 不含 chrono 字段: 关系本身不需要时间戳，时间戳由存储层（registry）管理。

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;

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

/// Unified entity model for all knowledge assets.
///
/// Replaces the repo-centric `RepoEntry` monopoly.  Each node carries
/// frontmatter (arbitrary key–value metadata) so that vault notes,
/// git repos, and external links can coexist in the same query/results.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub path: PathBuf,
    pub title: Option<String>,
    /// Raw frontmatter as key–value pairs.  For vault notes this is the
    /// parsed YAML header; for git repos it may contain `remote_origin`,
    /// `default_branch`, etc.
    pub frontmatter: HashMap<String, String>,
    pub tags: Vec<String>,
    pub outgoing_links: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Node {
    /// Convenience accessor for a frontmatter value.
    pub fn fm(&self, key: &str) -> Option<&str> {
        self.frontmatter.get(key).map(|s| s.as_str())
    }

    /// Whether this node is a git repo.
    pub fn is_repo(&self) -> bool {
        self.node_type == NodeType::GitRepo
    }

    /// Whether this node is a vault note.
    pub fn is_vault(&self) -> bool {
        self.node_type == NodeType::VaultNote
    }
}

/// A directed edge in the knowledge graph.
#[derive(Debug, Clone)]
pub struct Edge {
    pub from_id: String,
    pub to_id: String,
    pub edge_type: String,
    pub metadata: Option<String>,
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

    #[test]
    fn test_node_frontmatter_access() {
        let node = Node {
            id: "test".into(),
            node_type: NodeType::VaultNote,
            path: PathBuf::from("/tmp/test.md"),
            title: Some("Test".into()),
            frontmatter: {
                let mut m = HashMap::new();
                m.insert("status".into(), "draft".into());
                m
            },
            tags: vec![],
            outgoing_links: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(node.fm("status"), Some("draft"));
        assert_eq!(node.fm("missing"), None);
        assert!(node.is_vault());
    }
}
