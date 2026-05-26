// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! devbase-vault-frontmatter — Markdown frontmatter parser for vault notes.
//!
//! **提取日期**: 2026-05-01 (Workspace split)
//! **零内部耦合**: 此 crate 不依赖 devbase 任何内部模块，仅使用 std。
//! **职责**: 解析 Markdown 文件的 YAML frontmatter 头部，返回键值对。
//! **边界**: 输入 `&str`（文件内容），输出 `HashMap<String, String>`。不触及文件系统。
//!
//! 与 devbase 的关系: 被 devbase `vault/indexer` 调用，解析笔记元数据。
//!
//! Design decisions:
//! - 使用 `---` 分隔符: 兼容 Obsidian / Jekyll / Hugo 格式。
//! - 空值保留为空字符串: 避免 `None` 与 `""` 的歧义。
//! - 只解析第一个 frontmatter 块: 后续 `---` 视为正文。

use std::collections::HashMap;

mod parser;

/// Parsed frontmatter from a Markdown vault note.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frontmatter {
    pub id: Option<String>,
    pub title: Option<String>,
    pub repo: Option<String>,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub ai_context: Option<bool>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub date: Option<String>,
    pub raw: String,
    pub extra: HashMap<String, String>,
}

/// Extract YAML frontmatter from the top of a Markdown document.
///
/// Returns `(frontmatter, body_offset)` where `body_offset` is the byte index
/// at which the Markdown body begins (after the closing `---`).
pub fn extract_frontmatter(content: &str) -> Option<(Frontmatter, usize)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_open = &trimmed[3..];
    let close_pos = after_open.find("\n---")?;
    let raw = after_open[..close_pos].trim();
    let body_offset = trimmed.as_ptr() as usize - content.as_ptr() as usize + 3 + close_pos + 4;

    let fm = parser::parse_yaml_frontmatter(raw);
    Some((fm, body_offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_frontmatter() {
        assert!(extract_frontmatter("# Hello\nworld").is_none());
    }

    #[test]
    fn test_basic_yaml_frontmatter() {
        let md = "---\ntitle: Hello World\ntags: [rust, cli]\ndate: 2024-01-01\n---\n# Body\n";
        let (fm, offset) = extract_frontmatter(md).unwrap();
        assert_eq!(fm.title, Some("Hello World".to_string()));
        assert_eq!(fm.tags, vec!["rust", "cli"]);
        assert_eq!(fm.date, Some("2024-01-01".to_string()));
        assert!(md[offset..].trim_start().starts_with("# Body"));
    }

    #[test]
    fn test_multiline_list() {
        let md = "---\ntags:\n  - rust\n  - cli\n---\nbody\n";
        let (fm, _) = extract_frontmatter(md).unwrap();
        assert_eq!(fm.tags, vec!["rust", "cli"]);
    }

    #[test]
    fn test_quoted_strings() {
        let md = "---\ntitle: \"My Note\"\ntags: ['a', 'b']\n---\n";
        let (fm, _) = extract_frontmatter(md).unwrap();
        assert_eq!(fm.title, Some("My Note".to_string()));
        assert_eq!(fm.tags, vec!["a", "b"]);
    }

    #[test]
    fn test_extra_fields() {
        let md = "---\ntitle: X\ncategory: dev\n---\n";
        let (fm, _) = extract_frontmatter(md).unwrap();
        assert_eq!(fm.extra.get("category"), Some(&"dev".to_string()));
    }
}
