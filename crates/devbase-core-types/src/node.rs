// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::node_type::NodeType;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_type::NodeType;

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
